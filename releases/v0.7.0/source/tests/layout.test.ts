import assert from "node:assert/strict";
import test from "node:test";

import {
  clampColWidth,
  clampOverlayPos,
  COL_LIMITS,
  DEFAULT_COL_WIDTHS,
  parseColWidths,
} from "../src/layout.ts";

test("clampColWidth 限制在各列范围内", () => {
  assert.equal(clampColWidth("name", 50), COL_LIMITS.name.min);
  assert.equal(clampColWidth("name", 5000), COL_LIMITS.name.max);
  assert.equal(clampColWidth("size", 100), 100);
  // 非有限数一律回落默认值（而非夹到 max，避免坏值把列撑到极限）
  assert.equal(clampColWidth("path", Number.NaN), DEFAULT_COL_WIDTHS.path);
  assert.equal(clampColWidth("path", Number.POSITIVE_INFINITY), DEFAULT_COL_WIDTHS.path);
});

test("parseColWidths 对缺失/坏数据回落默认值", () => {
  assert.deepEqual(parseColWidths(null), DEFAULT_COL_WIDTHS);
  assert.deepEqual(parseColWidths(undefined), DEFAULT_COL_WIDTHS);
  assert.deepEqual(parseColWidths("nonsense"), DEFAULT_COL_WIDTHS);
  assert.deepEqual(parseColWidths(42), DEFAULT_COL_WIDTHS);
  // 缺字段：只覆盖出现的字段，其余保持默认
  const partial = parseColWidths({ name: 400 });
  assert.equal(partial.name, 400);
  assert.equal(partial.path, DEFAULT_COL_WIDTHS.path);
  // 类型错误字段被忽略，不污染其他字段
  const mixed = parseColWidths({ name: "wide", size: 120 });
  assert.equal(mixed.name, DEFAULT_COL_WIDTHS.name);
  assert.equal(mixed.size, 120);
  // 越界值被夹紧
  assert.equal(parseColWidths({ name: 99999 }).name, COL_LIMITS.name.max);
});

test("clampOverlayPos 无锚点时居中且不越界", () => {
  const pos = clampOverlayPos(undefined, { width: 246, height: 460 }, { width: 1000, height: 620 });
  assert.equal(pos.left, Math.round((1000 - 246) / 2));
  assert.equal(pos.top, Math.round((620 - 460) / 2));
});

test("clampOverlayPos 贴住锚点并在右下越界时收回", () => {
  const viewport = { width: 1000, height: 620 };
  const size = { width: 246, height: 460 };
  // 正常位置：不动
  assert.deepEqual(clampOverlayPos({ x: 100, y: 100 }, size, viewport), { left: 100, top: 100 });
  // 右下越界：收到边界内，仍留 8px 边距
  const br = clampOverlayPos({ x: 980, y: 600 }, size, viewport);
  assert.equal(br.left, viewport.width - size.width - 8);
  assert.equal(br.top, viewport.height - size.height - 8);
  // 负锚点：贴住左上边距
  assert.deepEqual(clampOverlayPos({ x: -50, y: -50 }, size, viewport), { left: 8, top: 8 });
});

test("clampOverlayPos 在窗口比浮层还小时不产生负偏移", () => {
  // 窗口 400x300 小于菜单 246x460（minHeight 480 场景下的小窗口）
  const pos = clampOverlayPos({ x: 350, y: 280 }, { width: 246, height: 460 }, { width: 400, height: 300 });
  assert.ok(pos.left >= 8, `left 应 >= 8，实际 ${pos.left}`);
  assert.ok(pos.top >= 8, `top 应 >= 8，实际 ${pos.top}`);
});
