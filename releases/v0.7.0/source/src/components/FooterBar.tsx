import { useState } from "react";
import { formatBytes, formatTime, type FileRow } from "../api";

interface Props {
  /** 当前选中项。状态栏常驻显示它的完整路径、大小与修改时间（对齐 Everything 的状态栏）。 */
  item?: FileRow | null;
  /** 索引条数（关键词搜索为全局总数，位置浏览为该范围根目录第一层条数）；无索引位置时不传 */
  total?: number;
  /** 最近一次查询耗时（毫秒）；没有执行查询（空搜索隐藏结果）时不传，只显示索引条数 */
  elapsed?: number;
}

export default function FooterBar({ item, total, elapsed }: Props) {
  const [tip, setTip] = useState<string | null>(null);
  // 字段常驻：无选中项（结果为空、加载中）时用「—」占位。
  // 此前整段不渲染，字段会突然消失、右侧计数跟着左右跳动。
  // 无选中项时不堆「—」，改为淡色字段名 + 一句提示，空搜索时更像说明而不是缺数据。
  const sizeText = item ? (item.isDir ? "文件夹" : formatBytes(item.size)) : "大小";
  const timeText = item ? formatTime(item.modifiedMs) : "修改时间";

  return (
    <div className="footerbar">
      {/* 只留最常用的两个按键提示，放最前；完整按键说明在预览浮层里 */}
      <span className="footerbar-hints">
        <kbd>↑</kbd>
        <kbd>↓</kbd> 选择
        <i />
        <kbd>Esc</kbd> 清空 / 关闭
      </span>
      {/* 标签必须放在 RTL 省略元素之外：路径过长时从左侧省略，
          若把标签放进同一元素，标签会被带到路径右侧 */}
      <span className="footerbar-path-wrap">
        <span className="footerbar-path-label">路径：</span>
        <span
          className={item ? "footerbar-path" : "footerbar-path idle"}
          // 不用原生 title：长路径只显示成一整行，常被屏幕截断；改为自绘可换行的提示
          onMouseEnter={(e) => {
            const el = e.currentTarget;
            // 只有路径被省略（放不下）时才需要提示
            setTip(item && el.scrollWidth > el.clientWidth ? item.path : null);
          }}
          onMouseLeave={() => setTip(null)}
        >
          {/* 外层 rtl 只负责从左侧省略；<bdo dir="ltr"> 固定内部顺序，
              否则 \\server 开头的反斜杠、结尾的括号等中性字符会被 rtl 挪到另一端 */}
          <bdo dir="ltr">{item?.path ?? "选中文件后显示完整路径"}</bdo>
        </span>
        {tip && item ? <div className="footerbar-tip">{tip}</div> : null}
      </span>
      <span className={item ? "footerbar-meta" : "footerbar-meta idle"}>
        {sizeText} ｜ {timeText}
      </span>
      {total === undefined ? null : (
        <span className="footerbar-status">
          {`索引 ${total.toLocaleString()} 条`}
          {elapsed === undefined ? "" : ` · 用时 ${elapsed} ms`}
        </span>
      )}
    </div>
  );
}
