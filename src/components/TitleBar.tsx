import { GearIcon, CloseIcon, MaximizeIcon, MinimizeIcon } from "./Icons";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openManage } from "../api";

interface Props {
  onNotice?: (message: string) => void;
}

export default function TitleBar({ onNotice }: Props) {
  const appWindow = getCurrentWindow();
  return (
    <div className="titlebar" data-tauri-drag-region>
      <img src="/findflow.svg" alt="" className="titlebar-logo" draggable={false} data-tauri-drag-region />
      <span className="titlebar-name" data-tauri-drag-region>FindFlow</span>
      <div className="titlebar-actions">
        <button
          className="titlebar-btn"
          title="打开管理窗（设置 / 分析 / 批量 / 任务）"
          onClick={() => {
            if (onNotice) onNotice("正在打开管理窗…");
            openManage().catch((e) => onNotice?.(String(e)));
          }}
        >
          <GearIcon />
        </button>
        <button className="titlebar-btn" title="最小化" onClick={() => void appWindow.minimize()}>
          <MinimizeIcon />
        </button>
        <button className="titlebar-btn" title="最大化 / 还原" onClick={() => void appWindow.toggleMaximize()}>
          <MaximizeIcon />
        </button>
        <button className="titlebar-btn titlebar-close" title="关闭" onClick={() => void appWindow.close()}>
          <CloseIcon />
        </button>
      </div>
    </div>
  );
}
