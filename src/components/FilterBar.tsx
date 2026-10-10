import type { Density } from "../types";

export const FILTERS = [
  { id: "all", label: "全部" },
  { id: "document", label: "文档" },
  { id: "folder", label: "文件夹" },
  { id: "image", label: "图片" },
  { id: "audio", label: "音乐" },
  { id: "video", label: "视频" },
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
  /** 命中总数（有查询条件时为真实命中数，空搜索浏览时为索引总数） */
  count?: number;
  /** 结果达到返回上限（实际命中可能更多） */
  limited?: boolean;
  /** 是否带查询条件（关键词或筛选）：带条件时计数是「命中 N 项」，否则是浏览范围的「共 N 项」，
   *  与状态栏「索引 N 条」（索引总量）区分开 */
  filtered?: boolean;
  /** 本次实际列出的条数；仅在 truncated 时用于说明「共 N 项 · 显示前 M 项」 */
  shown?: number;
}

export default function FilterBar({
  filter,
  onFilterChange,
  density,
  onDensityChange,
  count,
  limited,
  filtered,
  shown,
}: Props) {
  const noun = filtered ? "命中" : "共";
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
            title={
              limited
                ? `共命中 ${count.toLocaleString()} 项，此处仅列出最相关的前 ${(shown ?? count).toLocaleString()} 项，可继续输入缩小范围`
                : undefined
            }
          >
            {limited && shown !== undefined
              ? `${noun} ${count.toLocaleString()} 项 · 显示前 ${shown.toLocaleString()}`
              : `${noun} ${count.toLocaleString()} 项`}
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
