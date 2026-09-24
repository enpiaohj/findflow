import { useEffect, useRef } from "react";
import type { Density } from "../types";
import KindBadge from "./KindBadge";
import { LocateIcon, MoreIcon, OpenIcon } from "./Icons";
import { actionLocate, actionOpen, formatBytes, formatTime, type FileRow } from "../api";

interface Props {
  items: FileRow[];
  density: Density;
  selectedIndex: number;
  onSelect: (index: number) => void;
  onNotice: (message: string) => void;
  onOpen: (item: FileRow) => void;
  onMenu: (item: FileRow) => void;
}

export default function ResultList({
  items,
  density,
  selectedIndex,
  onSelect,
  onNotice,
  onOpen,
  onMenu,
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
          尝试更短的关键词；或在管理窗「索引与搜索」中把该目录加入索引范围。
        </p>
      </div>
    );
  }

  const openItem = (item: FileRow) => {
    actionOpen(item.path)
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
          >
            <div className={"result-thumb thumb-" + item.kind}>
              <KindBadge kind={item.kind} large />
            </div>
            <div className="result-card-name">{item.name}</div>
            <div className="result-card-meta">
              {formatBytes(item.size)} ｜ {formatTime(item.modifiedMs)}
            </div>
          </button>
        ))}
      </div>
    );
  }

  const comfortable = density === "comfortable";
  return (
    <div className="result-list" ref={listRef}>
      {items.map((item, index) => (
        <div
          key={item.fileId}
          className={"result-row row-" + density + (index === selectedIndex ? " result-selected" : "")}
          onClick={() => onSelect(index)}
          onDoubleClick={() => openItem(item)}
        >
          <div className="result-main">
            <KindBadge kind={item.kind} />
            <div className="result-text">
              <div className="result-name">
                {item.name}
                {item.favorite ? <span className="fav-star"> ★</span> : null}
              </div>
              {comfortable && item.snippet ? (
                <div className="result-summary">{item.snippet}</div>
              ) : null}
            </div>
          </div>
          {!comfortable ? (
            <>
              <div className="result-path">{item.path}</div>
              <div className="result-modified">{formatTime(item.modifiedMs)}</div>
            </>
          ) : null}
          {comfortable ? (
            <div className="result-meta-col">
              <div>{formatTime(item.modifiedMs)}</div>
              <div>{formatBytes(item.size)}</div>
            </div>
          ) : (
            <div className="result-size">{formatBytes(item.size)}</div>
          )}
          <div className="result-actions">
            <button
              title="快速预览（Space）"
              onClick={(e) => {
                e.stopPropagation();
                onOpen(item);
              }}
            >
              <OpenIcon />
            </button>
            <button
              title="在文件夹中定位"
              onClick={(e) => {
                e.stopPropagation();
                actionLocate(item.path)
                  .then((m) => onNotice(m))
                  .catch((err) => onNotice(String(err)));
              }}
            >
              <LocateIcon />
            </button>
            <button
              title="更多操作"
              onClick={(e) => {
                e.stopPropagation();
                onMenu(item);
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
