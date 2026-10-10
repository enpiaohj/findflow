/** 把条目所在的文件夹写成 `path:` 过滤词：路径含空格时加引号（后端支持 `path:"D:\My Docs"`）。 */
export function folderFilterOf(path: string): string | null {
  const i = path.lastIndexOf("\\");
  if (i <= 0) return null;
  // 盘符根目录（D:\x → D:\）保留末尾反斜杠
  const dir = i === 2 && path[1] === ":" ? path.slice(0, 3) : path.slice(0, i);
  return /\s/.test(dir) ? `path:"${dir}"` : `path:${dir}`;
}

/** 在查询串里设置路径限定：已有 `path:` 条件（不含 `-path:`）则替换，否则追加。 */
export function withPathFilter(query: string, filter: string): string {
  const tokens = query.match(/-?path:"[^"]*"|\S+/g) ?? [];
  const kept = tokens.filter((t) => !/^path:/i.test(t));
  return [...kept, filter].join(" ").trim();
}

/**
 * 搜索框内容若整体是一个绝对路径（如 D:\Temp、D:/Temp/、带引号的 "D:\My Docs"、UNC 的 \\srv\share），返回去掉引号与首尾空白的路径；
 * 否则返回 null。只认「盘符 + 分隔符」或 UNC 开头，避免把 `d:` 之类的过滤语法误当成路径。
 */
export function pathQueryOf(query: string): string | null {
  const q = query.trim().replace(/^"(.*)"$/, "$1").trim();
  if (/^[A-Za-z]:[\\/]/.test(q) || /^\\\\[^\\/]+/.test(q)) return q;
  return null;
}
