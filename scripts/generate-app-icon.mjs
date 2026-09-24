// 生成 FindFlow 应用图标源图：1024×1024 PNG，蓝色圆角底 + 白色放大镜。
// 仅依赖 Node 内置模块。用法：node scripts/generate-app-icon.mjs [输出路径]
import { deflateSync } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SIZE = 1024;
const CORNER = 190;
const BG_TOP = [37, 99, 235]; // #2563EB
const BG_BOTTOM = [12, 74, 200];

const DEFAULT_OUT = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../src-tauri/app-icon.png"
);
const OUT = process.argv[2] ?? DEFAULT_OUT;

function clamp(v, lo, hi) {
  return Math.max(lo, Math.min(hi, v));
}

// 有符号距离场的 1px 抗锯齿覆盖度。
function coverage(sdf) {
  return clamp(0.5 - sdf, 0, 1);
}

function roundedRectSdf(x, y, half, r) {
  const dx = Math.abs(x) - (half - r);
  const dy = Math.abs(y) - (half - r);
  const ax = Math.max(dx, 0);
  const ay = Math.max(dy, 0);
  return Math.hypot(ax, ay) + Math.min(Math.max(dx, dy), 0) - r;
}

function ringSdf(x, y, cx, cy, radius, halfWidth) {
  return Math.abs(Math.hypot(x - cx, y - cy) - radius) - halfWidth;
}

function capsuleSdf(x, y, ax, ay, bx, by, r) {
  const abx = bx - ax;
  const aby = by - ay;
  const t = clamp(
    ((x - ax) * abx + (y - ay) * aby) / (abx * abx + aby * aby),
    0,
    1
  );
  return Math.hypot(x - (ax + abx * t), y - (ay + aby * t)) - r;
}

const rows = [];
for (let py = 0; py < SIZE; py++) {
  const row = Buffer.alloc(SIZE * 4 + 1);
  row[0] = 0; // PNG 行滤波：None
  for (let px = 0; px < SIZE; px++) {
    // 以画布中心为原点的几何坐标
    const gx = px + 0.5 - SIZE / 2;
    const gy = py + 0.5 - SIZE / 2;
    const bg = coverage(roundedRectSdf(gx, gy, SIZE / 2 - 2, CORNER));
    const fg = Math.max(
      coverage(ringSdf(gx, gy, -45, -60, 190, 47)),
      coverage(capsuleSdf(gx, gy, 95, 10, 250, 165, 36))
    );
    const t = (py + 0.5) / SIZE;
    const o = 1 + px * 4;
    for (let c = 0; c < 3; c++) {
      const base = BG_TOP[c] + (BG_BOTTOM[c] - BG_TOP[c]) * t;
      row[o + c] = Math.round(base + (255 - base) * fg);
    }
    row[o + 3] = Math.round(bg * 255);
  }
  rows.push(row);
}

const CRC_TABLE = (() => {
  const table = new Int32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c;
  }
  return table;
})();

function crc32(buf) {
  let c = 0xffffffff;
  for (const b of buf) c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body), 0);
  return Buffer.concat([len, body, crc]);
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(SIZE, 0);
ihdr.writeUInt32BE(SIZE, 4);
ihdr[8] = 8; // 位深
ihdr[9] = 6; // 颜色类型：RGBA

const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(Buffer.concat(rows))),
  chunk("IEND", Buffer.alloc(0)),
]);

mkdirSync(dirname(OUT), { recursive: true });
writeFileSync(OUT, png);
console.log(`已生成应用图标源图：${OUT}（${SIZE}×${SIZE}）`);
