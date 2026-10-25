/** 更新下载进度的纯计算（便于单元测试）。 */

export interface UpdateProgress {
  downloaded: number;
  /** 服务器未给出总大小时为 null */
  total: number | null;
  /** 下载完成、即将启动安装程序 */
  finished: boolean;
}

/** 进度比例 0–1；总大小未知时返回 null（界面显示不定进度条）。 */
export function progressRatio(p: UpdateProgress): number | null {
  if (!p.total || p.total <= 0) return null;
  return Math.min(1, Math.max(0, p.downloaded / p.total));
}

function mb(bytes: number): string {
  return (bytes / (1024 * 1024)).toFixed(1);
}

/** 进度文案：「正在下载 3.2 / 13.5 MB（24%）」；总大小未知时只报已下载量。 */
export function progressText(p: UpdateProgress): string {
  if (p.finished) return "下载完成，正在启动安装程序…";
  const ratio = progressRatio(p);
  if (ratio === null) return `正在下载 ${mb(p.downloaded)} MB…`;
  return `正在下载 ${mb(p.downloaded)} / ${mb(p.total as number)} MB（${Math.round(ratio * 100)}%）`;
}
