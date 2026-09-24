import { useEffect, useRef } from "react";
import type { Density, FileItem } from "../types";
import KindBadge from "./KindBadge";
import { LocateIcon, MoreIcon, OpenIcon } from "./Icons";

interface Props {
  items: FileItem[];
  density: Density;
  selectedIndex: number;
  onSelect: (index: number) => void;
  onNotice: (message: string) => void;
}

export default function ResultList({ items, density, selectedIndex, onSelect, onNotice }: Props) {
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
        <p className="result-empty-hint">尝试更短的关键词，或在设置中把该目录加入索引范围。</p>
      </div>
    );
  }

  if (density === "large") {
    return (
      <div className="result-grid" ref={listRef}>
        {items.map((item, index) => (
          <button
            key={item.id}
            className={"result-card" + (index === selectedIndex ? " result-selected" : "")}
            onClick={() => onSelect(index)}
            onDoubleClick={() => onNotice("打开动作将在索引引擎接入后可用")}
          >
            <div className={"result-thumb thumb-" + item.kind}>
              <KindBadge kind={item.kind} large />
            </div>
            <div className="result-card-name">{item.name}</div>
            <div className="result-card-meta">
              {item.size} ｜ {item.modified}
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
          key={item.id}
          className={"result-row row-" + density + (index === selectedIndex ? " result-selected" : "")}
          onClick={() => onSelect(index)}
          onDoubleClick={() => onNotice("打开动作将在索引引擎接入后可用")}
        >
          <div className="result-main">
            <KindBadge kind={item.kind} />
            <div className="result-text">
              <div className="result-name">{item.name}</div>
              {comfortable && item.summary ? <div className="result-summary">{item.summary}</div> : null}
            </div>
          </div>
          {!comfortable ? (
            <>
              <div className="result-path">{item.path}</div>
              <div className="result-modified">{item.modified}</div>
            </>
          ) : null}
          {comfortable ? (
            <div className="result-meta-col">
              <div>{item.modified}</div>
              <div>{item.size}</div>
            </div>
          ) : (
            <div className="result-size">{item.size}</div>
          )}
          <div className="result-actions">
            <button
              title="打开"
              onClick={(e) => {
                e.stopPropagation();
                onNotice("打开动作将在索引引擎接入后可用");
              }}
            >
              <OpenIcon />
            </button>
            <button
              title="定位"
              onClick={(e) => {
                e.stopPropagation();
                onNotice("定位动作将在索引引擎接入后可用");
              }}
            >
              <LocateIcon />
            </button>
            <button
              title="更多"
              onClick={(e) => {
                e.stopPropagation();
                onNotice("更多菜单将在后续版本接入");
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
