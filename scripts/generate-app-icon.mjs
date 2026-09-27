// 生成 FindFlow 全套应用图标（方案 A · 流光透镜）。
//
// 源文件（src-tauri/icons/source/）：
//   app-icon.svg     主图，48px 及以上尺寸由它缩放
//   app-icon-32.svg  24–40px 像素对齐简化图
//   app-icon-16.svg  16–20px 像素对齐简化图
//
// 流程：Chrome / Edge 无头模式按精确像素渲染 SVG（透明背景）→ 输出 src-tauri/app-icon.png（1024）
//   → 调用 `npx tauri icon` 生成 icns / Store 等全套 → 用像素对齐小图重新组装 icon.ico 与 32x32.png
//   → 同步界面标题栏 / favicon 用的 public/findflow.svg。
//
// 仅依赖 Node 内置模块与本机 Chrome / Edge（可用 CHROME_PATH / EDGE_PATH 指定）。
// 用法：node scripts/generate-app-icon.mjs
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const SRC = join(ROOT, "src-tauri", "icons", "source");
const ICONS = join(ROOT, "src-tauri", "icons");

// 无头浏览器：优先 Chrome（Edge 无头模式在与前台 Edge 共存时偶发不退出），可用环境变量指定
const BROWSER =
  process.env.CHROME_PATH ??
  process.env.EDGE_PATH ??
  [
    "C:/Program Files/Google/Chrome/Application/chrome.exe",
    "C:/Program Files (x86)/Google/Chrome/Application/chrome.exe",
    "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
    "C:/Program Files/Microsoft/Edge/Application/msedge.exe",
  ].find(existsSync);
if (!BROWSER) {
  console.error("未找到 Chrome 或 Edge，请通过 CHROME_PATH / EDGE_PATH 指定浏览器路径");
  process.exit(1);
}

const tmp = mkdtempSync(join(tmpdir(), "findflow-icon-"));

/** 把 SVG 按 size×size 像素渲染为透明背景 PNG，返回文件内容。 */
function render(svgFile, size) {
  const svg = readFileSync(join(SRC, svgFile), "utf8").replace(
    /<svg /,
    `<svg width="${size}" height="${size}" style="display:block" `
  );
  const html = join(tmp, `${svgFile}-${size}.html`);
  const png = join(tmp, `${svgFile}-${size}.png`);
  writeFileSync(
    html,
    `<!doctype html><html><body style="margin:0;background:transparent">${svg}</body></html>`
  );
  execFileSync(
    BROWSER,
    [
      "--headless=new",
      "--disable-gpu",
      "--hide-scrollbars",
      "--default-background-color=00000000",
      "--force-device-scale-factor=1",
      "--no-first-run",
      // 独立配置目录：否则会挂到上一次残留的无头进程上导致卡住
      `--user-data-dir=${join(tmp, `profile-${size}-${svgFile}`)}`,
      `--window-size=${size},${size}`,
      `--screenshot=${png}`,
      pathToFileURL(html).href,
    ],
    { stdio: "ignore", timeout: 60_000 }
  );
  return readFileSync(png);
}

/** 以 PNG 条目组装 ICO（Vista 起支持），entries: [{size, png}]。 */
function buildIco(entries) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(entries.length, 4);
  const dir = Buffer.alloc(16 * entries.length);
  let offset = 6 + dir.length;
  entries.forEach(({ size, png }, i) => {
    const o = i * 16;
    dir.writeUInt8(size >= 256 ? 0 : size, o);
    dir.writeUInt8(size >= 256 ? 0 : size, o + 1);
    dir.writeUInt8(0, o + 2);
    dir.writeUInt8(0, o + 3);
    dir.writeUInt16LE(1, o + 4);
    dir.writeUInt16LE(32, o + 6);
    dir.writeUInt32LE(png.length, o + 8);
    dir.writeUInt32LE(offset, o + 12);
    offset += png.length;
  });
  return Buffer.concat([header, dir, ...entries.map((e) => e.png)]);
}

try {
  // 1. 1024 源图 → Tauri 全套图标
  const master = join(ROOT, "src-tauri", "app-icon.png");
  writeFileSync(master, render("app-icon.svg", 1024));
  console.log("已生成", master);
  execFileSync("npx", ["tauri", "icon", master], { cwd: ROOT, stdio: "inherit", shell: true });
  // 仅面向 Windows：移除 tauri icon 附带生成的移动端图标目录
  for (const d of ["android", "ios"]) rmSync(join(ICONS, d), { recursive: true, force: true });

  // 2. 小尺寸使用像素对齐简化图，重新组装 icon.ico
  const plan = [
    [16, "app-icon-16.svg"],
    [20, "app-icon-16.svg"],
    [24, "app-icon-32.svg"],
    [32, "app-icon-32.svg"],
    [40, "app-icon-32.svg"],
    [48, "app-icon.svg"],
    [64, "app-icon.svg"],
    [256, "app-icon.svg"],
  ];
  const entries = plan.map(([size, file]) => ({ size, png: render(file, size) }));
  writeFileSync(join(ICONS, "icon.ico"), buildIco(entries));
  writeFileSync(join(ICONS, "32x32.png"), entries.find((e) => e.size === 32).png);
  console.log("已用像素对齐小图重建 icon.ico（16–256）与 32x32.png");

  // 3. 界面标题栏与 favicon 显示尺寸约 20px，使用简化图
  copyFileSync(join(SRC, "app-icon-32.svg"), join(ROOT, "public", "findflow.svg"));
  console.log("已同步 public/findflow.svg");
} finally {
  rmSync(tmp, { recursive: true, force: true });
}
