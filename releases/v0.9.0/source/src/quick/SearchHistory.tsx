import { useEffect, useRef } from "react";
import { CloseIcon, SearchIcon } from "../components/Icons";

interface Props {
  items: string[];
  onPick: (term: string) => void;
  onRemove: (term: string) => void;
  onClear: () => void;
  onClose: () => void;
}

/** 搜索框历史下拉：聚焦且无输入词时显示；项可删除，底部可清空。 */
export default function SearchHistory({ items, onPick, onRemove, onClear, onClose }: Props) {
  const boxRef = useRef<HTMLDivElement>(null);

  // 点击下拉外部时关闭（输入框聚焦保持，不影响继续输入）
  useEffect(() => {
    const onPointerDown = (event: PointerEvent) => {
      if (!boxRef.current?.parentElement?.contains(event.target as Node)) onClose();
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [onClose]);

  if (items.length === 0) return null;

  return (
    <div className="history-pop" ref={boxRef}>
      <div className="history-list">
        {items.map((term) => (
          <div key={term} className="history-item">
            <button className="history-pick" onMouseDown={() => onPick(term)} title={term}>
              <span className="history-icon">
                <SearchIcon size={14} />
              </span>
              <span className="history-term">{term}</span>
            </button>
            <button
              className="history-remove"
              title="删除此记录"
              aria-label={`删除历史记录 ${term}`}
              onMouseDown={(e) => {
                e.stopPropagation();
                onRemove(term);
              }}
            >
              <CloseIcon size={12} />
            </button>
          </div>
        ))}
      </div>
      <div className="history-foot">
        <button className="history-clear" onMouseDown={onClear}>
          清空搜索历史
        </button>
      </div>
    </div>
  );
}
