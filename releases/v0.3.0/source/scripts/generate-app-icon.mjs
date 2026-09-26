// 生成 FindFlow 应用图标源图：1024×1024 PNG。
// 设计：对角渐变蓝底 + 白色放大镜（带柔和投影）+ 左上高光，2× 超采样抗锯齿。
// 仅依赖 Node 内置模块。用法：node scripts/generate-app-icon.mjs [输出路径]
import { deflateSync } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const OUT_SIZE = 1024; // 最终输出尺寸
const SS = 2; // 超采样倍率
const SIZE = OUT_SIZE * SS;
const CORNER = 440; // 圆角半径（2048 空间）
const HALF = SIZE / 2;

// 渐变端色（左上 → 右下）
const GRAD_TL = [91, 155, 255]; // #5B9BFF
const GRAD_BR = [18, 62, 176]; // #123EB0

// 放大镜几何（2048 空间）
const RING_C = [880, 860];
const RING_R = 320;
const RING_T = 104; // 环厚度
const H_A = [1105, 1085]; // 手柄起点
const H_B = [1408, 1388]; // 手柄终点
const H_R = 84; // 手柄半径
const SHADOW_OFF = [58, 78]; // 投影偏移
const SHADOW_SIGMA = 110;
const SHADOW_ALPHA = 0.34;

const DEFAULT_OUT = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../src-tauri/app-icon.png"
);
const OUT = process.argv[2] ?? DEFAULT_OUT;

const clamp01 = (v) => Math.max(0, Math.min(1, v));
const cov = (sdf) => clamp01(0.5 - sdf);

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
  const t = clamp01(((x - ax) * abx + (y - ay) * aby) / (abx * abx + aby * aby));
  return Math.hypot(x - (ax + abx * t), y - (ay + aby * t)) - r;
}

// 放大镜主体（环 + 手柄）的有符号距离
function magnifierSdf(x, y) {
  const ring = ringSdf(x, y, RING_C[0], RING_C[1], RING_R, RING_T / 2);
  const handle = capsuleSdf(x, y, H_A[0], H_A[1], H_B[0], H_B[1], H_R);
  return Math.min(ring, handle);
}

const rows = [];
for (let py = 0; py < SIZE; py++) {
  const row = Buffer.alloc(SIZE * 4 + 1);
  row[0] = 0; // PNG 行滤波：None
  for (let px = 0; px < SIZE; px++) {
    // 2× 超采样
    let aSum = 0;
    let rSum = 0;
    let gSum = 0;
    let bSum = 0;
    for (let sy = 0; sy < SS; sy++) {
      for (let sx = 0; sx < SS; sx++) {
        const x = px + (sx + 0.5) / SS;
        const y = py + (sy + 0.5) / SS;
        const gx = x - HALF;
        const gy = y - HALF;

        const bg = cov(roundedRectSdf(gx, gy, HALF - 8, CORNER));
        // 对角渐变
        const t = clamp01((x + y) / (2 * SIZE));
        let br = GRAD_TL[0] + (GRAD_BR[0] - GRAD_TL[0]) * t;
        let bgc = GRAD_TL[1] + (GRAD_BR[1] - GRAD_TL[1]) * t;
        let bb = GRAD_TL[2] + (GRAD_BR[2] - GRAD_TL[2]) * t;
        // 左上柔和高光
        const hl = Math.exp(
          -(((x - 620) / 950) ** 2 + ((y - 520) / 950) ** 2)
        );
        br += (255 - br) * 0.14 * hl;
        bgc += (255 - bgc) * 0.14 * hl;
        bb += (255 - bb) * 0.14 * hl;

        // 柔和投影（放大镜形状右下偏移，高斯衰减）
        const shSdf = magnifierSdf(x - SHADOW_OFF[0], y - SHADOW_OFF[1]);
        const sh = SHADOW_ALPHA * Math.exp(-(shSdf * shSdf) / (2 * SHADOW_SIGMA * SHADOW_SIGMA));
        br *= 1 - sh * 0.55;
        bgc *= 1 - sh * 0.55;
        bb *= 1 - sh * 0.55;

        // 白色放大镜
        const fg = cov(magnifierSdf(x, y));
        rSum += br + (255 - br) * fg;
        gSum += bgc + (255 - bgc) * fg;
        bSum += bb + (255 - bb) * fg;
        aSum += bg;
      }
    }
    const n = SS * SS;
    const o = 1 + px * 4;
    row[o] = Math.round(rSum / n);
    row[o + 1] = Math.round(gSum / n);
    row[o + 2] = Math.round(bSum / n);
    row[o + 3] = Math.round((aSum / n) * 255);
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
ihdr[8] = 8;
ihdr[9] = 6;

// 2× 超采样降到输出尺寸（箱式滤波）
console.log("2048行1024列中心:", JSON.stringify(Array.from(rows[1024].slice(1+1024*4,1+1024*4+4))), "下行:", JSON.stringify(Array.from(rows[1025].slice(1+1024*4,1+1024*4+4))));
console.log("2048中心像素:", JSON.stringify(Array.from(rows[1024].slice(1+1024*4, 1+1024*4+4))), "2048边缘:", JSON.stringify(Array.from(rows[1024].slice(1, 5))), "行数:", rows.length);

function downsample(buf) {
  const out = [];
  for (let py = 0; py < OUT_SIZE; py++) {
    const row = Buffer.alloc(OUT_SIZE * 4 + 1);
    row[0] = 0;
    for (let px = 0; px < OUT_SIZE; px++) {
      for (let c = 0; c < 4; c++) {
        let sum = 0;
        for (let dy = 0; dy < SS; dy++) {
          for (let dx = 0; dx < SS; dx++) {
            const sy = py * SS + dy;
            const sx = px * SS + dx;
            sum += buf[sy][1 + (sx * 4 + c)];
          }
        }
        row[1 + px * 4 + c] = Math.round(sum / (SS * SS));
      }
    }
    if (py === 512) console.log("降采样后512行样本:", JSON.stringify(Array.from(row.slice(1+512*4, 1+512*4+4))));
    out.push(row);
  }
  return out;
}

const finalRows = downsample(rows);
const raw = Buffer.concat(finalRows);

const ihdrOut = Buffer.from(ihdr);
ihdrOut.writeUInt32BE(OUT_SIZE, 0);
ihdrOut.writeUInt32BE(OUT_SIZE, 4);

const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", ihdrOut),
  chunk("IDAT", deflateSync(raw)),
  chunk("IEND", Buffer.alloc(0)),
]);

mkdirSync(dirname(OUT), { recursive: true });
writeFileSync(OUT, png);
console.log(`已生成应用图标源图：${OUT}（${OUT_SIZE}×${OUT_SIZE}）`);
