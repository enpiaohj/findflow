// 生成应用内更新用的 latest.json。
//
// 更新器按「windows-x86_64-<安装包类型>」优先、「windows-x86_64」兜底的顺序找下载项。
// 只写兜底项（指向 NSIS 安装包）时，用 MSI 安装的用户点更新会装上 NSIS 版，变成两份并存；
// 所以这里为 NSIS 与 MSI 各写一条，兜底项沿用 NSIS。两种安装包都必须已签名（.sig）。
//
// 用法：node scripts/make-latest-json.mjs --version 0.8.2 --notes "更新说明" [--dir releases/v0.8.2] [--date 2026-10-10T12:00:00Z]

import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { join } from "node:path";

function arg(name, fallback) {
  const i = process.argv.indexOf(`--${name}`);
  return i > 0 && process.argv[i + 1] ? process.argv[i + 1] : fallback;
}

const version = arg("version");
const notes = arg("notes");
if (!version || !/^\d+\.\d+\.\d+$/.test(version) || !notes) {
  console.error('用法：node scripts/make-latest-json.mjs --version X.Y.Z --notes "更新说明" [--dir 目录] [--date ISO时间]');
  process.exit(2);
}
const dir = arg("dir", join("releases", `v${version}`));
const date = arg("date", new Date().toISOString().replace(/\.\d{3}Z$/, "Z"));
const base = `https://github.com/enpiaohj/findflow/releases/download/v${version}`;

function entry(file) {
  const sigPath = join(dir, `${file}.sig`);
  if (!existsSync(join(dir, file))) throw new Error(`缺少安装包：${join(dir, file)}`);
  if (!existsSync(sigPath)) throw new Error(`缺少签名：${sigPath}（先用 tauri signer sign 签名）`);
  return { signature: readFileSync(sigPath, "utf8").trim(), url: `${base}/${file}` };
}

try {
  const nsis = entry(`FindFlow-v${version}-win-x64-setup.exe`);
  const msi = entry(`FindFlow-v${version}-win-x64.msi`);
  const manifest = {
    version,
    notes,
    pub_date: date,
    platforms: {
      "windows-x86_64-nsis": nsis,
      "windows-x86_64-msi": msi,
      // 兜底：无法识别安装包类型的旧版本使用 NSIS
      "windows-x86_64": nsis,
    },
  };
  const out = join(dir, "latest.json");
  writeFileSync(out, JSON.stringify(manifest, null, 2) + "\n", "utf8");
  console.log(`已写入 ${out}：${Object.keys(manifest.platforms).join("、")}`);
} catch (e) {
  console.error(String(e.message ?? e));
  process.exit(1);
}
