import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  actionOpen,
  actionLocate,
  actionCopyPath,
  actionCopyFiles,
  actionOpenWith,
  actionProperties,
  actionOpenTerminal,
  actionOpenVscode,
  actionRename,
  favoriteToggle,
  type FileRow,
} from "../api";
import { useConfirm } from "../components/ConfirmDialog";
import { clampOverlayPos } from "../layout";
import { recycleItem } from "./itemActions";

interface Props {
  item: FileRow | null;
  pos?: { x: number; y: number };
  onClose: () => void;
  onNotice: (message: string) => void;
  onRefresh: () => void;
  /** 由菜单栏触发时直接进入重命名 */
  initial?: "rename";
}

export default function ResultMenu({ item, pos, onClose, onNotice, onRefresh, initial }: Props) {
  const [renaming, setRenaming] = useState(initial === "rename");
  const [newName, setNewName] = useState("");
  const menuRef = useRef<HTMLDivElement>(null);
  const [menuSize, setMenuSize] = useState<{ width: number; height: number } | null>(null);
  const { confirm, confirmEl } = useConfirm();

  useEffect(() => {
    if (item) setNewName(item.name);
    const onDoc = (e: MouseEvent) => {
      // 确认框内的点击不视为「菜单外部」，否则会在确认前卸载菜单
      const t = e.target as HTMLElement | null;
      if (t?.closest(".confirm-overlay")) return;
      if (menuRef.current && !menuRef.current.contains(t)) onClose();
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [item, onClose]);

  // 菜单高度随内容变化（菜单项 / 重命名输入），量出实际尺寸再定位，
  // 避免用固定估值导致矮窗口下菜单越界或压住触发点。
  useLayoutEffect(() => {
    const el = menuRef.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    setMenuSize((prev) =>
      prev && prev.width === r.width && prev.height === r.height ? prev : { width: r.width, height: r.height }
    );
  }, [renaming]);

  // 打开后焦点落在第一项，直接 ↑↓ / Enter 即可操作；退出重命名后也回到菜单项
  useEffect(() => {
    if (!renaming) menuRef.current?.querySelector<HTMLButtonElement>(".menu-item")?.focus();
  }, [renaming, item?.fileId]);

  if (!item) return null;
  const path = item.path;

  /** 执行动作并提示结果。`refresh` 仅在动作改变了结果集（收藏 / 重命名）时才需要，
   *  否则会让列表无谓地重查重排，正在加载的缩略图也跟着重来。 */
  const run = (p: Promise<string>, refresh = false) =>
    p
      .then((m) => {
        onNotice(m);
        if (refresh) onRefresh();
      })
      .catch((e) => onNotice(String(e)));

  /** 打开文件：先隐藏搜索窗再打开，避免启动器窗口压在刚打开的程序上面
   *  （与回车键打开的行为一致；开启「失焦自动隐藏」时效果相同，关闭时才有差别）。 */
  const openItem = () => {
    void getCurrentWindow().hide();
    run(actionOpen(path));
  };

  /** 重命名：路径变了，需要刷新结果列表才能看到新名字。 */
  const doRename = () =>
    run(
      actionRename(path, newName).then(() => {
        onClose();
        return "已重命名";
      }),
      true
    );

  const doRecycle = async () => {
    try {
      const message = await recycleItem(item, confirm);
      if (message === null) return;
      onNotice(message);
      onRefresh();
      onClose();
    } catch (e) {
      onNotice(String(e));
    }
  };

  // 分组：打开类 / 复制类 / 管理类；组间画分隔线，最常用的放最上面。
  type MenuItem = { label: string; action: () => void };
  const menuGroups: MenuItem[][] = [
    [
      { label: "打开", action: openItem },
      { label: "在文件夹中定位", action: () => run(actionLocate(path)) },
      // 「打开方式」只对文件有意义，文件夹没有可选的关联程序
      ...(item.isDir ? [] : [{ label: "打开方式…", action: () => run(actionOpenWith(path)) }]),
    ],
    [
      // 「复制文件」等同系统 Ctrl+C，最常用，排在最前
      { label: item.isDir ? "复制文件夹" : "复制文件", action: () => run(actionCopyFiles([path])) },
      { label: "复制完整路径", action: () => run(actionCopyPath(path, "full")) },
      { label: item.isDir ? "复制文件夹名" : "复制文件名", action: () => run(actionCopyPath(path, "name")) },
      { label: "复制父目录", action: () => run(actionCopyPath(path, "parent")) },
      { label: "复制 Markdown 链接", action: () => run(actionCopyPath(path, "markdown")) },
    ],
    [
      { label: "重命名…", action: () => setRenaming(true) },
      {
        label: item.favorite ? "取消收藏" : "收藏",
        action: () => run(favoriteToggle(item.fileId).then(() => "收藏已更新"), true),
      },
      { label: "在终端打开", action: () => run(actionOpenTerminal(path)) },
      { label: "在 VS Code 打开", action: () => run(actionOpenVscode(path)) },
      { label: "属性", action: () => run(actionProperties(path)) },
    ],
  ];

  /** 键盘操作：↑↓ 在菜单项间移动焦点（首尾循环），Enter 由按钮原生触发。 */
  const onMenuKeyDown = (e: React.KeyboardEvent) => {
    if (renaming || (e.key !== "ArrowDown" && e.key !== "ArrowUp")) return;
    const buttons = Array.from(menuRef.current?.querySelectorAll<HTMLButtonElement>(".menu-item") ?? []);
    if (buttons.length === 0) return;
    e.preventDefault();
    e.stopPropagation();
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const next =
      e.key === "ArrowDown" ? (at + 1) % buttons.length : (at - 1 + buttons.length) % buttons.length;
    buttons[next].focus();
  };

  // 跟随触发位置弹出，并收在窗口范围内（尺寸未量出前先按估值定位）
  const { left, top } = clampOverlayPos(
    pos,
    menuSize ?? { width: 246, height: 460 },
    { width: window.innerWidth, height: window.innerHeight }
  );

  return (
    <>
      {confirmEl}
      <div className="menu-overlay" onClick={onClose}>
        <div
        className="menu-panel"
        ref={menuRef}
        onKeyDown={onMenuKeyDown}
        style={pos ? { left, top, transform: "none" } : undefined}
        onClick={(e) => e.stopPropagation()}
      >
        {renaming ? (
          <div className="menu-rename">
            <input
              value={newName}
              autoFocus
              onChange={(e) => setNewName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  doRename();
                }
                if (e.key === "Escape") {
                  // 只退出重命名、保留菜单：不拦下事件的话会继续冒泡到
                  // QuickApp 的全局按键处理，把整个菜单一并关掉。
                  e.stopPropagation();
                  setRenaming(false);
                }
              }}
            />
            <button onClick={doRename}>确定</button>
          </div>
        ) : (
          <>
            {menuGroups.map((group, gi) => (
              <div key={gi}>
                {gi > 0 ? <div className="menu-divider" /> : null}
                {group.map((m) => (
                  <button key={m.label} className="menu-item" onClick={m.action}>
                    {m.label}
                  </button>
                ))}
              </div>
            ))}
            <div className="menu-divider" />
            <button className="menu-item menu-danger" onClick={() => void doRecycle()}>
              移至回收站
            </button>
          </>
        )}
        </div>
      </div>
    </>
  );
}
