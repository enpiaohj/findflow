import { actionRecycle, issueDeleteToken, type FileRow } from "../api";

type Confirm = (message: string, sub?: string) => Promise<boolean>;

/**
 * 确认后把条目移至回收站（右键菜单与菜单栏共用）。
 * 返回要提示给用户的文案；用户取消时返回 null。
 */
export async function recycleItem(item: FileRow, confirm: Confirm): Promise<string | null> {
  if (!(await confirm(`将「${item.name}」移至回收站？`, "可在系统回收站中还原。"))) return null;
  const target = [{ fileId: item.fileId, path: item.path }];
  const token = await issueDeleteToken(target);
  const results = await actionRecycle(target, token);
  const failed = results.filter((r) => r.status !== "ok");
  return failed.length === 0 ? "已移至回收站" : failed[0].reason || "操作失败";
}
