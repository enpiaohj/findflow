//! 系统文件图标与缩略图（与资源管理器一致）。
//!
//! 通过 `IShellItemImageFactory` 取图：图标按扩展名缓存（exe / lnk 等自带图标的类型按路径缓存），
//! 缩略图按「路径 + 修改时间 + 尺寸」缓存。Shell 扩展可能很慢甚至卡死，因此取图放在独立的
//! STA 工作线程上，调用方带超时等待；工作线程超时即被替换，不会拖住界面命令。

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use base64::Engine;
use serde::Deserialize;

/// 自带独立图标、必须按路径取图的扩展名。
const PER_PATH_EXTS: &[&str] = &["exe", "lnk", "ico", "url", "msc", "cpl", "scr", "appref-ms"];

/// 缓存键：与前端 `iconKey` 保持一致。
pub fn icon_key(path: &str, ext: &str, is_dir: bool, size: u32) -> String {
    let ext = ext.to_ascii_lowercase();
    if is_dir {
        format!("dir@{size}")
    } else if PER_PATH_EXTS.contains(&ext.as_str()) {
        format!("path:{}@{size}", path.to_lowercase())
    } else {
        format!("ext:{ext}@{size}")
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Icon,
    Thumb,
}

struct Job {
    path: String,
    size: u32,
    mode: Mode,
    reply: Sender<Option<String>>,
}

/// 可替换的工作线程：请求超时视为卡死，换一个新线程继续服务。
struct Worker {
    tx: Mutex<Sender<Job>>,
}

impl Worker {
    fn new() -> Self {
        Self {
            tx: Mutex::new(spawn_worker()),
        }
    }

    fn request(&self, path: &str, size: u32, mode: Mode, timeout: Duration) -> Option<String> {
        let (reply, rx) = channel();
        let job = Job {
            path: path.to_string(),
            size,
            mode,
            reply,
        };
        if self.tx.lock().unwrap().send(job).is_err() {
            *self.tx.lock().unwrap() = spawn_worker();
            return None;
        }
        match rx.recv_timeout(timeout) {
            Ok(v) => v,
            Err(_) => {
                tracing::warn!("取图超时，已替换取图线程");
                *self.tx.lock().unwrap() = spawn_worker();
                None
            }
        }
    }
}

fn spawn_worker() -> Sender<Job> {
    let (tx, rx) = channel::<Job>();
    std::thread::Builder::new()
        .name("shell-image".into())
        .spawn(move || {
            #[cfg(windows)]
            unsafe {
                use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            }
            for job in rx {
                let result = std::panic::catch_unwind(|| extract(&job.path, job.size, job.mode))
                    .ok()
                    .flatten();
                let _ = job.reply.send(result);
            }
        })
        .ok();
    tx
}

fn icon_worker() -> &'static Worker {
    static W: OnceLock<Worker> = OnceLock::new();
    W.get_or_init(Worker::new)
}

fn thumb_worker() -> &'static Worker {
    static W: OnceLock<Worker> = OnceLock::new();
    W.get_or_init(Worker::new)
}

fn icon_cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    static C: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

fn thumb_cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    static C: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 缩略图缓存条目上限（约数十 MB），超出后整体清空重建。
const THUMB_CACHE_MAX: usize = 400;

#[cfg(windows)]
fn extract(path: &str, size: u32, mode: Mode) -> Option<String> {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::SIZE;
    use windows::Win32::UI::Shell::{
        IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF, SIIGBF_BIGGERSIZEOK,
        SIIGBF_ICONONLY, SIIGBF_THUMBNAILONLY,
    };

    let factory: IShellItemImageFactory =
        unsafe { SHCreateItemFromParsingName(&HSTRING::from(path), None) }.ok()?;
    let flags = match mode {
        Mode::Icon => SIIGBF_ICONONLY,
        Mode::Thumb => SIIGBF(SIIGBF_THUMBNAILONLY.0 | SIIGBF_BIGGERSIZEOK.0),
    };
    let side = size as i32;
    let hbmp = unsafe { factory.GetImage(SIZE { cx: side, cy: side }, flags) }.ok()?;
    let rgba = bitmap_to_rgba(hbmp);
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::DeleteObject(hbmp);
    }
    let (w, h, mut pixels) = rgba?;
    fix_alpha(&mut pixels);
    // 缩略图含透明像素（如透明 PNG）时必须保留 Alpha，否则 JPEG 会把透明区域变成黑底
    let has_alpha = pixels.chunks_exact(4).any(|p| p[3] < 255);
    let img = image::RgbaImage::from_raw(w, h, pixels)?;
    let mut img = image::DynamicImage::ImageRgba8(img);
    // 缩略图可能按 BIGGERSIZEOK 返回更大的图，缩到请求尺寸以内再编码
    if w > size || h > size {
        img = img.resize(size, size, image::imageops::FilterType::Triangle);
    }
    let mut buf = Cursor::new(Vec::new());
    let mime = match mode {
        Mode::Icon => {
            img.write_to(&mut buf, image::ImageFormat::Png).ok()?;
            "image/png"
        }
        Mode::Thumb if has_alpha => {
            img.write_to(&mut buf, image::ImageFormat::Png).ok()?;
            "image/png"
        }
        Mode::Thumb => {
            let rgb = img.to_rgb8();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 85)
                .encode_image(&rgb)
                .ok()?;
            "image/jpeg"
        }
    };
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(buf.into_inner())
    ))
}

#[cfg(not(windows))]
fn extract(_path: &str, _size: u32, _mode: Mode) -> Option<String> {
    None
}

/// 读取 32 位 HBITMAP 为自上而下的 RGBA 像素。
#[cfg(windows)]
fn bitmap_to_rgba(
    hbmp: windows::Win32::Graphics::Gdi::HBITMAP,
) -> Option<(u32, u32, Vec<u8>)> {
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    unsafe {
        let mut bm = BITMAP::default();
        if GetObjectW(
            hbmp,
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bm as *mut _ as *mut core::ffi::c_void),
        ) == 0
        {
            return None;
        }
        let (w, h) = (bm.bmWidth, bm.bmHeight.abs());
        if w <= 0 || h <= 0 {
            return None;
        }
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // 负高度 = 自上而下
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut buf = vec![0u8; (w * h * 4) as usize];
        let dc = CreateCompatibleDC(None);
        let lines = GetDIBits(
            dc,
            hbmp,
            0,
            h as u32,
            Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
            &mut info,
            DIB_RGB_COLORS,
        );
        let _ = DeleteDC(dc);
        if lines == 0 {
            return None;
        }
        // BGRA → RGBA
        for px in buf.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
        Some((w as u32, h as u32, buf))
    }
}

/// Shell 返回的位图可能是预乘 Alpha，也可能完全没有 Alpha（alpha 全 0 的不透明图）。
fn fix_alpha(pixels: &mut [u8]) {
    if pixels.chunks_exact(4).all(|p| p[3] == 0) {
        for p in pixels.chunks_exact_mut(4) {
            p[3] = 255;
        }
        return;
    }
    // 存在颜色分量大于 alpha 的像素说明不是预乘格式，保持原样
    let premultiplied = pixels
        .chunks_exact(4)
        .all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3]);
    if !premultiplied {
        return;
    }
    for p in pixels.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IconRequest {
    pub path: String,
    pub ext: String,
    pub is_dir: bool,
}

/// 批量取文件图标：返回 缓存键 → data URL（取不到为 null，前端回退为内置图标）。
#[tauri::command]
pub async fn file_icons(
    items: Vec<IconRequest>,
    size: u32,
) -> Result<HashMap<String, Option<String>>, String> {
    let size = size.clamp(16, 256);
    tauri::async_runtime::spawn_blocking(move || {
        let mut out = HashMap::new();
        for item in items {
            let key = icon_key(&item.path, &item.ext, item.is_dir, size);
            if out.contains_key(&key) {
                continue;
            }
            if let Some(v) = icon_cache().lock().unwrap().get(&key) {
                out.insert(key, v.clone());
                continue;
            }
            let v = icon_worker().request(&item.path, size, Mode::Icon, Duration::from_secs(3));
            // 失败的结果也缓存（本次运行内不再重试），避免反复卡在同一个坏扩展上
            icon_cache().lock().unwrap().insert(key.clone(), v.clone());
            out.insert(key, v);
        }
        out
    })
    .await
    .map_err(|e| e.to_string())
}

/// 取单个文件缩略图（图片 / 视频 / 文档等由系统缩略图提供程序生成）；无缩略图返回 null。
#[tauri::command]
pub async fn file_thumbnail(
    path: String,
    modified_ms: i64,
    size: u32,
) -> Result<Option<String>, String> {
    let size = size.clamp(32, 512);
    let key = format!("{}|{modified_ms}|{size}", path.to_lowercase());
    if let Some(v) = thumb_cache().lock().unwrap().get(&key) {
        return Ok(v.clone());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let v = thumb_worker().request(&path, size, Mode::Thumb, Duration::from_secs(8));
        let mut cache = thumb_cache().lock().unwrap();
        // 超出上限时只淘汰最旧的一批（约 1/4），而不是整体清空——
        // 全清会让当前视口内所有缩略图重新生成，滚动时反复抖动。
        if cache.len() >= THUMB_CACHE_MAX {
            let drop_n = THUMB_CACHE_MAX / 4;
            let keys: Vec<String> = cache.keys().take(drop_n).cloned().collect();
            for k in keys {
                cache.remove(&k);
            }
        }
        cache.insert(key, v.clone());
        v
    })
    .await
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_keys_group_by_extension_except_self_iconed_types() {
        assert_eq!(icon_key(r"D:\a\x.DOCX", "DOCX", false, 32), "ext:docx@32");
        assert_eq!(icon_key(r"D:\b\y.docx", "docx", false, 32), "ext:docx@32");
        assert_eq!(icon_key(r"D:\App\Tool.exe", "exe", false, 32), r"path:d:\app\tool.exe@32");
        assert_eq!(icon_key(r"D:\a", "", true, 48), "dir@48");
    }

    #[test]
    fn unpremultiplies_only_premultiplied_icons() {
        // 预乘：半透明红 (128,0,0,128) → (255,0,0,128)
        let mut px = vec![128, 0, 0, 128, 0, 0, 0, 255];
        fix_alpha(&mut px);
        assert_eq!(&px[..4], &[255, 0, 0, 128]);
        // alpha 全 0 视为不透明
        let mut opaque = vec![10, 20, 30, 0];
        fix_alpha(&mut opaque);
        assert_eq!(opaque, vec![10, 20, 30, 255]);
    }

    #[test]
    fn extracts_icon_for_real_file() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("sample.txt");
        std::fs::write(&f, b"x").unwrap();
        #[cfg(windows)]
        unsafe {
            use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        let url = extract(&f.to_string_lossy(), 32, Mode::Icon).expect("应能取到系统图标");
        assert!(url.starts_with("data:image/png;base64,"));
    }
}
