/** 空搜索且隐藏结果列表时的引导：给出几个可点击的搜索写法，避免中间大片空白。 */

export const TIPS: { query: string; desc: string }[] = [
  { query: "报告 年度", desc: "多个词需同时命中" },
  { query: "*.pdf", desc: "通配符：所有 PDF" },
  { query: "ext:docx", desc: "按扩展名" },
  { query: "size:>100MB", desc: "按大小" },
  { query: "modified:7d", desc: "最近 7 天修改" },
  { query: "-type:图片", desc: "排除某类文件" },
  { query: "path:D:\\Temp", desc: "限定在某个文件夹" },
  { query: "-path:node_modules", desc: "排除某个文件夹" },
];

interface Props {
  onPick: (query: string) => void;
}

export default function IdleHint({ onPick }: Props) {
  return (
    <div className="idle-hint">
      <p className="idle-hint-title">输入关键词开始搜索</p>
      <p className="idle-hint-sub">也可以试试这些写法，点一下填入搜索框</p>
      <div className="idle-hint-tips">
        {TIPS.map((tip) => (
          <button key={tip.query} className="idle-tip" onClick={() => onPick(tip.query)}>
            <code>{tip.query}</code>
            <span>{tip.desc}</span>
          </button>
        ))}
      </div>
    </div>
  );
}
