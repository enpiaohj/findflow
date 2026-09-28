import type { Density } from "../types";

export const FILTERS = [
  { id: "all", label: "全部" },
  { id: "document", label: "文档" },
  { id: "folder", label: "文件夹" },
  { id: "image", label: "图片" },
  { id: "recent", label: "最近" },
] as const;

export type FilterId = (typeof FILTERS)[number]["id"];

const DENSITY_LABELS: Record<Density, string> = {
  compact: "紧凑",
  comfortable: "舒适",
  large: "大图标",
};

const DENSITIES: Density[] = ["compact", "comfortable", "large"];

interface Props {
  filter: FilterId;
  onFilterChange: (filter: FilterId) => void;
  density: Density;
  onDensityChange: (density: Density) => void;
  /** 结果条数；空搜索隐藏结果时不传，不显示计数 */
  count?: number;
  /** 结果达到返回上限（实际命中可能更多） */
  limited?: boolean;
}

export default function FilterBar({
  filter,
  onFilterChange,
  density,
  onDensityChange,
  count,
  limited,
}: Props) {
  return (
    <div className="filterbar">
      <div className="filterbar-chips">
        {FILTERS.map((f) => (
          <button
            key={f.id}
            className={"chip" + (filter === f.id ? " chip-active" : "")}
            onClick={() => onFilterChange(f.id)}
          >
            {f.label}
          </button>
        ))}
      </div>
      <div className="filterbar-meta">
        {count === undefined ? null : (
          <span
            className="filterbar-count"
            title={limited ? "仅显示最相关的前若干项，可继续输入缩小范围" : undefined}
          >
            {limited ? `前 ${count} 项` : `共 ${count} 项`}
          </span>
        )}
        <div className="density-switch" role="group" aria-label="显示密度">
          {DENSITIES.map((d) => (
            <button
              key={d}
              className={"density-btn" + (density === d ? " density-btn-active" : "")}
              title={DENSITY_LABELS[d]}
              onClick={() => onDensityChange(d)}
            >
              <span className={"density-glyph density-glyph-" + d} />
            </button>
          ))}
        </div>
        <span className="density-label">{DENSITY_LABELS[density]}</span>
      </div>
    </div>
  );
}
