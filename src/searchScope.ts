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
