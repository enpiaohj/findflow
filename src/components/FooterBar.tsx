export default function FooterBar() {
  return (
    <div className="footerbar">
      <span className="footerbar-hints">
        <kbd>↑</kbd>
        <kbd>↓</kbd> 选择
        <i />
        <kbd>Esc</kbd> 清空 / 关闭
        <i />
        <kbd>Ctrl+L</kbd> 聚焦搜索框
      </span>
      <span className="footerbar-note">演示数据 · 索引引擎尚未接入</span>
    </div>
  );
}
