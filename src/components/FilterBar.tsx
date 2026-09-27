import type { ScopeInfo } from "../api";
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
  count: number;
  /** 结果达到返回上限（实际命中可能更多） */
  limited: boolean;
  /** 索引位置；locationId 为 0 表示所有位置 */
  scopes: ScopeInfo[];
  locationId: number;
  onLocationChange: (scopeId: number) => void;
  /** 已输入搜索词：关键词搜索始终搜全部位置，位置选择置灰 */
  locationDisabled: boolean;
}

export default function FilterBar({
  filter,
  onFilterChange,
  density,
  onDensityChange,
  count,
  limited,
  scopes,
  locationId,
  onLocationChange,
  locationDisabled,
}: Props) {
  return (
    <div className="filterbar">
      <div className="filterbar-chips">
        <select
          className="location-select"
          value={locationId}
          disabled={locationDisabled}
          onChange={(e) => onLocationChange(Number(e.target.value))}
          title={locationDisabled ? "关键词搜索始终搜全部位置；位置仅用于未输入时的列表" : "未输入搜索词时列表显示的位置"}
          aria-label="列表位置"
        >
          <option value={0}>所有位置</option>
          {scopes
            .filter((s) => s.enabled)
            .map((s) => (
              <option key={s.scopeId} value={s.scopeId}>
                {s.path}
              </option>
            ))}
        </select>
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
        <span className="filterbar-count" title={limited ? "仅显示最相关的前若干项，可继续输入缩小范围" : undefined}>
          {limited ? `前 ${count} 项` : `共 ${count} 项`}
        </span>
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
