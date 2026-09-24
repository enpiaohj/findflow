use std::fs::File;
use std::os::windows::io::AsRawHandle;
use std::path::Path;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{
    GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
};

/// NTFS 卷内稳定标识：卷序列号 + 文件索引（设计文档 §8.2）。
/// 路径只是可变属性；标识不可用时返回 None，自动关联降级为低置信度。
pub fn native_identity(path: &Path) -> Option<(u32, u64)> {
    let file = File::open(path).ok()?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    let handle = HANDLE(file.as_raw_handle());
    let ok = unsafe { GetFileInformationByHandle(handle, &mut info) };
    if ok.is_err() {
        return None;
    }
    let id = (info.nFileIndexHigh as u64) << 32 | info.nFileIndexLow as u64;
    Some((info.dwVolumeSerialNumber, id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_of_real_file() {
        let p = std::env::current_exe().unwrap();
        // exe 自身必然可读；结果取决于文件系统（NTFS 上应有值）。
        let _ = native_identity(&p);
    }
}
