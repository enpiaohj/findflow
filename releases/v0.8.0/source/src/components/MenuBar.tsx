import { useCallback, useEffect, useRef, useState } from "react";

export type MenuEntry =
  | {
      kind: "item";
      label: string;
      /** 右侧快捷键提示 */
      hint?: string;
      /** 单选 / 开关项的选中标记 */
      checked?: boolean;
      disabled?: boolean;
      danger?: boolean;
      onSelect: () => void;
    }
  | { kind: "sep" }
  | { kind: "title"; label: string };

export interface MenuDef {
  id: string;
  /** 显示名，如「文件(F)」 */
  label: string;
  /** Alt + 该字母打开 */
  accel: string;
  entries: MenuEntry[];
}

interface Props {
  menus: MenuDef[];
  /** 菜单关闭后把焦点还给搜索框 */
  onRestoreFocus: () => void;
  /** 菜单展开 / 收起（搜索窗的全局按键处理据此让路） */
  onOpenChange?: (open: boolean) => void;
}

/** 类似 Everything 的菜单栏：点击或 Alt+字母打开，悬停切换，↑↓←→ / Enter / Esc 键盘操作。 */
export default function MenuBar({ menus, onRestoreFocus, onOpenChange }: Props) {
  const [openId, setOpenId] = useState<string | null>(null);
  const barRef = useRef<HTMLDivElement>(null);

  const setOpen = useCallback(
    (id: string | null) => {
      setOpenId(id);
      onOpenChange?.(id !== null);
    },
    [onOpenChange],
  );

  const close = useCallback(
    (restoreFocus: boolean) => {
      setOpen(null);
      if (restoreFocus) onRestoreFocus();
    },
    [onRestoreFocus, setOpen],
  );

  // 点击菜单栏外部关闭
  useEffect(() => {
    if (!openId) return;
    const onDown = (e: MouseEvent) => {
      if (!barRef.current?.contains(e.target as Node)) setOpen(null);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [openId, setOpen]);

  // Alt + 字母打开对应菜单
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
      // 预览 / 确认框 / 右键菜单等覆盖层打开时不抢按键
      if (document.querySelector(".preview-overlay, .confirm-overlay, .menu-overlay")) return;
      const menu = menus.find((m) => m.accel.toLowerCase() === e.key.toLowerCase());
      if (!menu) return;
      e.preventDefault();
      e.stopPropagation();
      setOpen(menu.id);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [menus, setOpen]);

  // 展开后焦点落在第一项
  useEffect(() => {
    if (!openId) return;
    barRef.current?.querySelector<HTMLButtonElement>(".mb-item:not(:disabled)")?.focus();
  }, [openId]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (!openId) return;
    const idx = menus.findIndex((m) => m.id === openId);
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      close(true);
    } else if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
      e.preventDefault();
      e.stopPropagation();
      const step = e.key === "ArrowRight" ? 1 : -1;
      setOpen(menus[(idx + step + menus.length) % menus.length].id);
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      e.stopPropagation();
      const items = Array.from(barRef.current?.querySelectorAll<HTMLButtonElement>(".mb-item:not(:disabled)") ?? []);
      if (items.length === 0) return;
      const at = items.indexOf(document.activeElement as HTMLButtonElement);
      const next = e.key === "ArrowDown" ? (at + 1) % items.length : (at - 1 + items.length) % items.length;
      items[next].focus();
    } else if (e.key === "Enter" || e.key === " ") {
      // 由聚焦的按钮原生触发点击；阻止冒泡避免触发搜索窗的 Enter（打开文件）
      e.stopPropagation();
    }
  };

  return (
    <div className="menubar" ref={barRef} onKeyDown={onKeyDown} role="menubar">
      {menus.map((menu) => (
        <div key={menu.id} className="mb-slot">
          <button
            className={"mb-top" + (openId === menu.id ? " mb-top-open" : "")}
            aria-haspopup="menu"
            aria-expanded={openId === menu.id}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => (openId === menu.id ? close(true) : setOpen(menu.id))}
            onMouseEnter={() => openId && openId !== menu.id && setOpen(menu.id)}
          >
            {menu.label}
          </button>
          {openId === menu.id ? (
            <div className="mb-drop" role="menu">
              {menu.entries.map((entry, i) => {
                if (entry.kind === "sep") return <div key={i} className="mb-sep" />;
                if (entry.kind === "title") {
                  return (
                    <div key={i} className="mb-title">
                      {entry.label}
                    </div>
                  );
                }
                return (
                  <button
                    key={i}
                    role="menuitem"
                    className={"mb-item" + (entry.danger ? " mb-danger" : "")}
                    disabled={entry.disabled}
                    onClick={() => {
                      close(false);
                      entry.onSelect();
                    }}
                  >
                    <span className="mb-check">{entry.checked ? "✓" : ""}</span>
                    <span className="mb-label">{entry.label}</span>
                    {entry.hint ? <span className="mb-hint">{entry.hint}</span> : null}
                  </button>
                );
              })}
            </div>
          ) : null}
        </div>
      ))}
    </div>
  );
}
