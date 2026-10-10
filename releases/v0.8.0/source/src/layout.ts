/** 结果列表列宽与浮层定位的纯计算逻辑（便于单元测试）。 */

export interface ColWidths {
  name: number;
  path: number;
  modified: number;
  size: number;
}

/** 默认列宽，与旧版 flex 比例（38% / 30% / 128px / 70px）在常见窗口下观感一致。 */
export const DEFAULT_COL_WIDTHS: ColWidths = {
  name: 320,
  path: 260,
  modified: 128,
  size: 70,
};

/** 各列可拖拽范围：名称/路径可变幅度大，时间/大小只需小幅调整。 */
export const COL_LIMITS: Record<keyof ColWidths, { min: number; max: number }> = {
  name: { min: 120, max: 900 },
  path: { min: 90, max: 900 },
  modified: { min: 96, max: 260 },
  size: { min: 56, max: 200 },
};

export const COL_KEYS: (keyof ColWidths)[] = ["name", "path", "modified", "size"];

export function clampColWidth(col: keyof ColWidths, width: number): number {
  const { min, max } = COL_LIMITS[col];
  if (!Number.isFinite(width)) return DEFAULT_COL_WIDTHS[col];
  return Math.round(Math.min(max, Math.max(min, width)));
}

/**
 * 解析持久化的列宽设置。设置值可能缺失、类型错误或来自旧版本，
 * 任何非法字段都回落到默认值，保证界面不会因坏数据塌陷。
 */
export function parseColWidths(raw: unknown): ColWidths {
  if (!raw || typeof raw !== "object") return { ...DEFAULT_COL_WIDTHS };
  const source = raw as Record<string, unknown>;
  const out = { ...DEFAULT_COL_WIDTHS };
  for (const key of COL_KEYS) {
    const value = source[key];
    if (typeof value === "number") out[key] = clampColWidth(key, value);
  }
  return out;
}

export interface OverlaySize {
  width: number;
  height: number;
}

export interface Viewport {
  width: number;
  height: number;
}

/**
 * 把浮层收进窗口范围内。优先贴住触发点，空间不足时向左/上翻转，
 * 始终保留 margin 边距；窗口比浮层还小时退化为贴住左上角，不再出现负偏移。
 */
export function clampOverlayPos(
  anchor: { x: number; y: number } | undefined,
  size: OverlaySize,
  viewport: Viewport,
  margin = 8,
): { left: number; top: number } {
  const maxLeft = Math.max(margin, viewport.width - size.width - margin);
  const maxTop = Math.max(margin, viewport.height - size.height - margin);
  if (!anchor) {
    return {
      left: Math.max(margin, Math.round((viewport.width - size.width) / 2)),
      top: Math.max(margin, Math.round((viewport.height - size.height) / 2)),
    };
  }
  return {
    left: Math.min(Math.max(margin, anchor.x), maxLeft),
    top: Math.min(Math.max(margin, anchor.y), maxTop),
  };
}

/** 预览窗格宽度：默认值与可拖拽范围；列表至少保留 MIN_LIST_WIDTH，避免名称列被挤没。 */
export const DEFAULT_PANE_WIDTH = 340;
export const MIN_PANE_WIDTH = 240;
export const MAX_PANE_WIDTH = 640;
export const MIN_LIST_WIDTH = 300;

/** 把窗格宽度限制在可用范围内；`areaWidth` 为列表 + 窗格所在区域的总宽。 */
export function clampPaneWidth(width: number, areaWidth: number): number {
  if (!Number.isFinite(width)) return DEFAULT_PANE_WIDTH;
  const max = Math.max(MIN_PANE_WIDTH, Math.min(MAX_PANE_WIDTH, areaWidth - MIN_LIST_WIDTH));
  return Math.round(Math.min(max, Math.max(MIN_PANE_WIDTH, width)));
}
