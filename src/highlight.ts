/** 结果名称的命中词高亮：从查询串提取可高亮的关键词，并把文本切成「命中 / 非命中」片段。 */

export interface Segment {
  text: string;
  hit: boolean;
}

/**
 * 提取关键词：忽略 `key:value` 过滤器、`-排除词`，去掉通配符与引号；
 * 只保留纯文本词，并按长度降序去重（长词优先匹配）。
 */
export function queryTerms(query: string): string[] {
  const terms = new Set<string>();
  for (const raw of query.split(/\s+/)) {
    if (!raw || raw.startsWith("-") || /^[A-Za-z]+:/.test(raw)) continue;
    const t = raw.replace(/["'*?]/g, "");
    if (t) terms.add(t.toLowerCase());
  }
  return [...terms].sort((a, b) => b.length - a.length);
}

/** 按关键词（不区分大小写）切分文本；无关键词或无命中时返回单个非命中片段。 */
export function splitHighlight(text: string, terms: string[]): Segment[] {
  if (terms.length === 0 || !text) return [{ text, hit: false }];
  const lower = text.toLowerCase();
  const marks = new Array<boolean>(text.length).fill(false);
  for (const term of terms) {
    let from = 0;
    for (;;) {
      const at = lower.indexOf(term, from);
      if (at < 0) break;
      for (let i = at; i < at + term.length; i++) marks[i] = true;
      from = at + term.length;
    }
  }
  const out: Segment[] = [];
  let start = 0;
  for (let i = 1; i <= text.length; i++) {
    if (i === text.length || marks[i] !== marks[start]) {
      out.push({ text: text.slice(start, i), hit: marks[start] });
      start = i;
    }
  }
  return out;
}
