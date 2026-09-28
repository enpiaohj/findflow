interface Props {
  /** 索引条数（关键词搜索为全局总数，位置浏览为该范围根目录第一层条数） */
  total: number;
  /** 最近一次查询耗时（毫秒） */
  elapsed: number;
}

export default function FooterBar({ total, elapsed }: Props) {
  return (
    <div className="footerbar">
      <span className="footerbar-hints">
        <kbd>↑</kbd>
        <kbd>↓</kbd> 选择
        <i />
        <kbd>Space</kbd> 预览
        <i />
        <kbd>Enter</kbd> 打开
        <i />
        <kbd>Esc</kbd> 清空 / 关闭
      </span>
      <span className="footerbar-status">
        {`索引 ${total.toLocaleString()} 条 · 用时 ${elapsed} ms`}
      </span>
    </div>
  );
}
