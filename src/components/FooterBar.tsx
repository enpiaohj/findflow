export default function FooterBar() {
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
      <span className="footerbar-note">本地索引 · 搜索词不落盘</span>
    </div>
  );
}
