/**
 * 搜索历史纯逻辑：记录、去重、容量控制（供搜索窗与测试共用）。
 * 历史仅保存在本机设置库（ui.searchHistory），可随时关闭或清空。
 */

export const SEARCH_HISTORY_KEY = "ui.searchHistory";
export const SEARCH_HISTORY_ENABLED_KEY = "ui.searchHistoryEnabled";
export const SEARCH_HISTORY_MAX = 50;

/** 解析设置中的历史 JSON；非法值一律回退为空数组。 */
export function parseSearchHistory(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.filter((item): item is string => typeof item === "string" && item.trim().length > 0);
}

/**
 * 记录一条搜索词：去空白、与最近一条相同则不重复（保留原位置不动，避免"当前词"反复置顶），
 * 其余同名旧条目移除后置顶，超出容量丢弃最旧的。
 */
export function recordSearch(history: string[], term: string, max: number = SEARCH_HISTORY_MAX): string[] {
  const trimmed = term.trim();
  if (!trimmed) return history;
  if (history[0] === trimmed) return history;
  return [trimmed, ...history.filter((item) => item !== trimmed)].slice(0, max);
}

/** 删除指定的一条历史。 */
export function removeSearch(history: string[], term: string): string[] {
  return history.filter((item) => item !== term);
}
