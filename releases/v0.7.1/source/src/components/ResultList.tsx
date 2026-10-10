import { useCallback, useEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";
import type { Density } from "../types";
import FileIcon, { FileThumb } from "./FileIcon";
import { LocateIcon, MoreIcon, OpenIcon } from "./Icons";
import { actionLocate, actionOpen, formatBytes, formatTime, getSetting, setSetting, type FileRow } from "../api";
import { clampColWidth, DEFAULT_COL_WIDTHS, parseColWidths, type ColWidths } from "../layout";

interface Props {
  items: FileRow[];
  density: Density;
  selectedIndex: number;
  sort: string;
  onSort: (sort: string) => void;
  onSelect: (index: number) => void;
  onNotice: (message: string) => void;
  onMenu: (item: FileRow, pos?: { x: number; y: number }) => void;
  onOpenSettings?: () => void;
  /** 空列表的原因：无索引位置 / 首次查询未返回 / 当前筛选下无结果；缺省为关键词无匹配 */
  emptyReason?: "no-scope" | "loading" | "filtered";
}

/** 列宽持久化键（紧凑模式）。 */
const COL_WIDTHS_KEY = "ui.colWidths";

/** 所在目录（去掉文件名）。 */
function parentDir(path: string): string {
  const i = path.lastIndexOf("\\");
  if (i < 0) return path;
  // 盘符根目录保留反斜杠（D:\）
  return i <= 2 ? path.slice(0, i + 1) : path.slice(0, i);
}

/**
 * 目录显示为「前段 + 最后一级」：空间不足时只省略中间部分，
 * 盘符与最后一级目录（最有辨识度的两端）始终可见。
 */
function DirPath({ path }: { path: string }) {
  const dir = parentDir(path);
  const i = dir.lastIndexOf("\\");
  if (i <= 2) return <span className="path-tail">{dir}</span>;
  return (
    <>
      <span className="path-head">{dir.slice(0, i + 1)}</span>
      <span className="path-tail">{dir.slice(i + 1)}</span>
    </>
  );
}

const COLUMNS: { id: keyof ColWidths; label: string; className: string; sortable: boolean }[] = [
  { id: "name", label: "名称", className: "col-name", sortable: true },
  { id: "path", label: "路径", className: "col-path", sortable: false },
  { id: "modified", label: "修改时间", className: "col-modified", sortable: true },
  { id: "size", label: "大小", className: "col-size", sortable: true },
];

export default function ResultList({
  items,
  density,
  selectedIndex,
  sort,
  onSort,
  onSelect,
  onNotice,
  onMenu,
  onOpenSettings,
  emptyReason,
}: Props) {
  const listRef = useRef<HTMLDivElement>(null);
  const [widths, setWidths] = useState<ColWidths>(DEFAULT_COL_WIDTHS);
  const widthsRef = useRef(widths);
  const dragRef = useRef<{ col: keyof ColWidths; startX: number; startWidth: number } | null>(null);
  const dragCleanupRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    widthsRef.current = widths;
  }, [widths]);

  // 读取持久化的列宽；坏数据由 parseColWidths 回落默认值。
  useEffect(() => {
    getSetting<unknown>(COL_WIDTHS_KEY)
      .then((v) => {
        if (v) setWidths(parseColWidths(v));
      })
      .catch(() => undefined);
  }, []);

  // 拖拽过程中若组件卸载，清理挂在 window 上的监听与拖拽态。
  useEffect(() => () => dragCleanupRef.current?.(), []);

  useEffect(() => {
    listRef.current
      ?.querySelector<HTMLElement>(".result-selected")
      ?.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

  /** 开始拖拽某一列的右边界调整宽度。 */
  const startResize = useCallback((e: React.MouseEvent, col: keyof ColWidths) => {
    e.preventDefault();
    e.stopPropagation();
    dragRef.current = { col, startX: e.clientX, startWidth: widthsRef.current[col] };
    document.body.classList.add("col-resizing");

    const onMove = (ev: MouseEvent) => {
      const d = dragRef.current;
      if (!d) return;
      const next = clampColWidth(d.col, d.startWidth + (ev.clientX - d.startX));
      setWidths((prev) => (prev[d.col] === next ? prev : { ...prev, [d.col]: next }));
    };
    const onUp = () => {
      dragCleanupRef.current?.();
      // 松手后持久化一次，避免拖动过程中频繁写库
      setSetting(COL_WIDTHS_KEY, widthsRef.current).catch(() => undefined);
    };
    dragCleanupRef.current = () => {
      dragRef.current = null;
      document.body.classList.remove("col-resizing");
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      dragCleanupRef.current = null;
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }, []);

  if (items.length === 0) {
    // 首次查询还没返回：留白，避免闪一下「没有匹配的结果」
    if (emptyReason === "loading") return <div className="result-empty" />;
    const noScope = emptyReason === "no-scope";
    const filtered = emptyReason === "filtered";
    return (
      <div className="result-empty">
        <p className="result-empty-title">
          {noScope ? "没有启用的索引位置" : filtered ? "当前筛选下没有结果" : "没有匹配的结果"}
        </p>
        <p className="result-empty-hint">
          {noScope
            ? "请在设置中启用或添加一个索引位置。"
            : filtered
              ? "切换到「全部」，或换个关键词试试。"
              : "尝试更短的关键词，或先把常用目录加入索引范围。"}
        </p>
        {noScope && onOpenSettings ? (
          <button className="btn-primary result-empty-btn" onClick={onOpenSettings}>
            ＋ 添加索引范围
          </button>
        ) : null}
      </div>
    );
  }

  const openItem = (item: FileRow) => {
    actionOpen(item.path)
      .then((m) => onNotice(m))
      .catch((e) => onNotice(String(e)));
  };
  const locateItem = (item: FileRow) => {
    actionLocate(item.path)
      .then((m) => onNotice(m))
      .catch((e) => onNotice(String(e)));
  };

  if (density === "large") {
    return (
      <div className="result-grid" ref={listRef}>
        {items.map((item, index) => (
          <button
            key={item.fileId}
            className={"result-card" + (index === selectedIndex ? " result-selected" : "")}
            onClick={() => onSelect(index)}
            onDoubleClick={() => openItem(item)}
            onContextMenu={(e) => {
              e.preventDefault();
              onSelect(index);
              onMenu(item, { x: e.clientX, y: e.clientY });
            }}
          >
            <FileThumb
              path={item.path}
              ext={item.ext}
              isDir={item.isDir}
              kind={item.kind}
              modifiedMs={item.modifiedMs}
              height={112}
            />
            <div className="result-card-name" title={item.name}>
              {item.name}
            </div>
            <div className="result-card-meta">
              {item.isDir ? "文件夹" : formatBytes(item.size)} ｜ {formatTime(item.modifiedMs)}
            </div>
          </button>
        ))}
      </div>
    );
  }

  const comfortable = density === "comfortable";
  // 通过 CSS 变量把列宽下发给标题行与数据行，二者共用同一组值。
  const colVars = {
    "--col-name": `${widths.name}px`,
    "--col-path": `${widths.path}px`,
    "--col-modified": `${widths.modified}px`,
    "--col-size": `${widths.size}px`,
  } as CSSProperties;

  return (
    <div className="result-list" ref={listRef} style={colVars}>
      {!comfortable ? (
        <div className="result-header">
          {COLUMNS.map((c) => (
            <div
              key={c.id}
              className={c.className + (c.sortable ? " sortable" : "") + (sort === c.id ? " active" : "")}
              onClick={
                c.sortable
                  ? () => onSort(sort === c.id ? "relevance" : c.id)
                  : undefined
              }
              title={c.sortable ? (sort === c.id ? "再次点击恢复相关度排序" : `按${c.label}排序`) : undefined}
            >
              {c.label}
              {sort === c.id ? <span className="sort-arrow">{c.id === "name" ? "↑" : "↓"}</span> : null}
              <span
                className="col-resizer"
                role="separator"
                aria-orientation="vertical"
                aria-label={`调整${c.label}列宽`}
                title="拖动调整列宽，双击恢复默认"
                onMouseDown={(e) => startResize(e, c.id)}
                onClick={(e) => e.stopPropagation()}
                onDoubleClick={(e) => {
                  e.stopPropagation();
                  setWidths((prev) => {
                    const next = { ...prev, [c.id]: DEFAULT_COL_WIDTHS[c.id] };
                    setSetting(COL_WIDTHS_KEY, next).catch(() => undefined);
                    return next;
                  });
                }}
              />
            </div>
          ))}
          <div className="col-actions" />
        </div>
      ) : null}
      {items.map((item, index) => (
        <div
          key={item.fileId}
          className={"result-row row-" + density + (index === selectedIndex ? " result-selected" : "")}
          onClick={() => onSelect(index)}
          onDoubleClick={() => openItem(item)}
          onContextMenu={(e) => {
            e.preventDefault();
            onSelect(index);
            onMenu(item, { x: e.clientX, y: e.clientY });
          }}
        >
          <div className="result-main col-name">
            <FileIcon
              path={item.path}
              ext={item.ext}
              isDir={item.isDir}
              kind={item.kind}
              size={comfortable ? 32 : 20}
            />
            <div className="result-text">
              <div className="result-name" title={item.name}>
                {item.name}
                {item.favorite ? <span className="fav-star"> ★</span> : null}
              </div>
              {comfortable ? (
                <>
                  <div className="result-subpath" title={item.path}>
                    <DirPath path={item.path} />
                  </div>
                  {item.snippet ? <div className="result-summary">{item.snippet}</div> : null}
                </>
              ) : null}
            </div>
          </div>
          {!comfortable ? (
            <>
              <div className="result-path col-path" title={item.path}>
                <DirPath path={item.path} />
              </div>
              <div className="result-modified col-modified">{formatTime(item.modifiedMs)}</div>
              <div className="result-size col-size">{item.isDir ? "" : formatBytes(item.size)}</div>
            </>
          ) : (
            <div className="result-meta-col">
              <div>{formatTime(item.modifiedMs)}</div>
              <div>{item.isDir ? "文件夹" : formatBytes(item.size)}</div>
            </div>
          )}
          <div className={"result-actions col-actions" + (comfortable ? " labeled" : "")}>
            <button
              title="打开（Enter）"
              onClick={(e) => {
                e.stopPropagation();
                openItem(item);
              }}
            >
              <OpenIcon />
              {comfortable ? <span>打开</span> : null}
            </button>
            <button
              title="在文件夹中定位（Ctrl+Enter）"
              onClick={(e) => {
                e.stopPropagation();
                locateItem(item);
              }}
            >
              <LocateIcon />
              {comfortable ? <span>定位</span> : null}
            </button>
            <button
              title="更多操作"
              onClick={(e) => {
                e.stopPropagation();
                onSelect(index);
                const r = e.currentTarget.getBoundingClientRect();
                onMenu(item, { x: r.right - 230, y: r.bottom + 4 });
              }}
            >
              <MoreIcon />
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}
