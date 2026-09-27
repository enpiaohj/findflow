import { useEffect, useRef } from "react";
import type { Density } from "../types";
import FileIcon, { FileThumb } from "./FileIcon";
import { LocateIcon, MoreIcon, OpenIcon } from "./Icons";
import { actionLocate, actionOpen, formatBytes, formatTime, type FileRow } from "../api";

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
}

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

const COLUMNS: { id: string; label: string; className: string; sortable: boolean }[] = [
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
}: Props) {
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    listRef.current
      ?.querySelector<HTMLElement>(".result-selected")
      ?.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

  if (items.length === 0) {
    return (
      <div className="result-empty">
        <p className="result-empty-title">没有匹配的结果</p>
        <p className="result-empty-hint">
          尝试更短的关键词，或先把常用目录加入索引范围。
        </p>
        {onOpenSettings ? (
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
  return (
    <div className="result-list" ref={listRef}>
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
