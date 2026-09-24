import { useEffect, useRef, useState } from "react";
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
  issueDeleteToken,
  actionRecycle,
  favoriteToggle,
  fileByPath,
  type FileRow,
} from "../api";

interface Props {
  item: FileRow | null;
  pos?: { x: number; y: number };
  onClose: () => void;
  onNotice: (message: string) => void;
  onRefresh: () => void;
}

export default function ResultMenu({ item, pos, onClose, onNotice, onRefresh }: Props) {
  const [renaming, setRenaming] = useState(false);
  const [newName, setNewName] = useState("");
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (item) setNewName(item.name);
    const onDoc = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) onClose();
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [item, onClose]);

  if (!item) return null;
  const path = item.path;

  const run = (p: Promise<string>) =>
    p.then((m) => {
      onNotice(m);
      onRefresh();
    }).catch((e) => onNotice(String(e)));

  const doRecycle = async () => {
    const token = await issueDeleteToken([path]);
    const results = await actionRecycle([path], token);
    const failed = results.filter((r) => r.status !== "ok");
    onNotice(failed.length === 0 ? "已移至回收站" : `失败 ${failed.length} 项`);
    onRefresh();
    onClose();
  };

  const menuItems: { label: string; action: () => void }[] = [
    { label: "打开", action: () => run(actionOpen(path)) },
    { label: "在文件夹中定位", action: () => run(actionLocate(path)) },
    { label: "复制完整路径", action: () => run(actionCopyPath(path, "full")) },
    { label: "复制文件名", action: () => run(actionCopyPath(path, "name")) },
    { label: "复制父目录", action: () => run(actionCopyPath(path, "parent")) },
    { label: "复制 Markdown 链接", action: () => run(actionCopyPath(path, "markdown")) },
    { label: "复制文件", action: () => run(actionCopyFiles([path])) },
    { label: "重命名…", action: () => setRenaming(true) },
    { label: "打开方式…", action: () => run(actionOpenWith(path)) },
    { label: "在终端打开", action: () => run(actionOpenTerminal(path)) },
    { label: "在 VS Code 打开", action: () => run(actionOpenVscode(path)) },
    { label: item.favorite ? "取消收藏" : "收藏", action: () => run(favoriteToggle(item.fileId).then(() => "收藏已更新")) },
    { label: "属性", action: () => run(actionProperties(path)) },
  ];

  // 跟随触发位置弹出，并收在窗口范围内
  const left = pos ? Math.min(pos.x, window.innerWidth - 250) : window.innerWidth / 2 - 110;
  const top = pos ? Math.min(pos.y, window.innerHeight - 380) : window.innerHeight / 2 - 160;

  return (
    <div className="menu-overlay" onClick={onClose}>
      <div
        className="menu-panel"
        ref={menuRef}
        style={pos ? { left, top, transform: "none" } : undefined}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="menu-title">{item.name}</div>
        {renaming ? (
          <div className="menu-rename">
            <input
              value={newName}
              autoFocus
              onChange={(e) => setNewName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  run(
                    actionRename(path, newName).then(() => {
                      onClose();
                      return "已重命名";
                    })
                  );
                }
                if (e.key === "Escape") setRenaming(false);
              }}
            />
            <button
              onClick={() =>
                run(
                  actionRename(path, newName).then(() => {
                    onClose();
                    return "已重命名";
                  })
                )
              }
            >
              确定
            </button>
          </div>
        ) : (
          <>
            {menuItems.map((m) => (
              <button key={m.label} className="menu-item" onClick={m.action}>
                {m.label}
              </button>
            ))}
            <div className="menu-divider" />
            <button className="menu-item menu-danger" onClick={() => void doRecycle()}>
              移至回收站
            </button>
          </>
        )}
      </div>
    </div>
  );
}

export async function fileExists(path: string): Promise<boolean> {
  return (await fileByPath(path)) !== null;
}
