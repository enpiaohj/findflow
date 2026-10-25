import { GearIcon, CloseIcon, MaximizeIcon, MinimizeIcon } from "./Icons";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";

interface Props {
  onOpenSettings?: () => void;
}

export default function TitleBar({ onOpenSettings }: Props) {
  const appWindow = getCurrentWindow();
  const [version, setVersion] = useState("");

  useEffect(() => {
    getVersion()
      .then((v) => setVersion(v))
      .catch(() => undefined);
  }, []);
  return (
    <div className="titlebar" data-tauri-drag-region>
      <img src="/findflow.svg" alt="" className="titlebar-logo" draggable={false} data-tauri-drag-region />
      <span className="titlebar-name" data-tauri-drag-region>FindFlow</span>
      {version ? <span className="titlebar-version" data-tauri-drag-region>v{version}</span> : null}
      <div className="titlebar-actions">
        <button
          className="titlebar-btn"
          title="设置（Esc 返回搜索）"
          onClick={() => onOpenSettings?.()}
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
