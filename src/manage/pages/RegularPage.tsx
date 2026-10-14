import { useEffect, useState } from "react";
import type { PageCtx } from "../ManageBody";
import {
  getSetting,
  setSetting,
  autostartIsEnabled,
  autostartEnable,
  autostartDisable,
  traySetVisible,
  appExit,
} from "../../api";

/** 常规：应用启动、窗口行为与退出。 */
export default function RegularPage({ notice }: PageCtx) {
  const [blurHide, setBlurHide] = useState(true);
  const [clearOnHide, setClearOnHide] = useState(true);
  const [autostart, setAutostart] = useState(false);
  const [showTray, setShowTray] = useState(true);
  const [autoCalibrate, setAutoCalibrate] = useState(true);
  const [autoFulltext, setAutoFulltext] = useState(false);

  useEffect(() => {
    getSetting<boolean>("ui.blurHide").then((v) => v !== null && setBlurHide(v)).catch(() => undefined);
    getSetting<boolean>("ui.clearOnHide").then((v) => setClearOnHide(v !== false)).catch(() => undefined);
    getSetting<boolean>("ui.showTrayIcon")
      .then((v) => setShowTray(v !== null ? !!v : true))
      .catch(() => undefined);
    getSetting<boolean>("ui.autoCalibrate")
      .then((v) => setAutoCalibrate(v !== null ? !!v : true))
      .catch(() => undefined);
    getSetting<boolean>("ui.autoFulltext")
      .then((v) => setAutoFulltext(!!v))
      .catch(() => undefined);
    autostartIsEnabled().then(setAutostart).catch(() => undefined);
  }, []);

  const changeAutostart = (checked: boolean) => {
    (checked ? autostartEnable() : autostartDisable())
      .then(() => setAutostart(checked))
      .catch((e) => notice(String(e)));
  };

  const changeShowTray = (checked: boolean) => {
    setShowTray(checked);
    setSetting("ui.showTrayIcon", checked).catch((e) => notice(String(e)));
    traySetVisible(checked).catch((e) => notice(String(e)));
  };

  return (
    <div className="page">
      <h2>常规</h2>
      <p className="page-desc">应用启动方式与窗口行为。</p>

      <section className="card">
        <div className="card-head">
          <h3>启动</h3>
        </div>
        <div className="toggle-row">
          <div>
            <b>开机自动启动</b>
            <p className="muted">登录 Windows 后自动在后台运行，仅显示托盘图标，不弹出主窗口</p>
          </div>
          <input
            type="checkbox"
            checked={autostart}
            onChange={(e) => changeAutostart(e.target.checked)}
          />
        </div>
      </section>

      <section className="card">
        <div className="card-head">
          <h3>窗口</h3>
        </div>
        <div className="toggle-row">
          <div>
            <b>失焦自动隐藏</b>
            <p className="muted">搜索窗失去焦点后自动隐藏，不打断当前工作</p>
          </div>
          <input
            type="checkbox"
            checked={blurHide}
            onChange={(e) => {
              setBlurHide(e.target.checked);
              setSetting("ui.blurHide", e.target.checked).catch(() => undefined);
            }}
          />
        </div>
        <div className="toggle-row">
          <div>
            <b>隐藏时清空搜索词</b>
            <p className="muted">默认开启：主动隐藏或失焦自动隐藏时清空；打开文件时保留搜索，窗口仅失焦但未隐藏时也保留</p>
          </div>
          <input
            type="checkbox"
            checked={clearOnHide}
            onChange={(e) => {
              setClearOnHide(e.target.checked);
              setSetting("ui.clearOnHide", e.target.checked).catch(() => undefined);
            }}
          />
        </div>
        <div className="toggle-row">
          <div>
            <b>显示托盘图标</b>
            <p className="muted">
              关闭后托盘区不再显示图标；仍可用全局快捷键呼出窗口，或回到本页重新打开
            </p>
          </div>
          <input
            type="checkbox"
            checked={showTray}
            onChange={(e) => changeShowTray(e.target.checked)}
          />
        </div>
      </section>

      <section className="card">
        <div className="card-head">
          <h3>索引</h3>
        </div>
        <div className="toggle-row">
          <div>
            <b>自动定期校准</b>
            <p className="muted">
              每天在后台校准一次索引，补上实时监听遗漏的改动（休眠、网络共享、快速改动等）。
              网络共享范围不做实时监听，只能靠它保持同步；关闭后需手动校准。
            </p>
          </div>
          <input
            type="checkbox"
            checked={autoCalibrate}
            onChange={(e) => {
              setAutoCalibrate(e.target.checked);
              setSetting("ui.autoCalibrate", e.target.checked).catch(() => undefined);
            }}
          />
        </div>
        <div className="toggle-row">
          <div>
            <b>自动提取正文</b>
            <p className="muted">
              在后台逐步提取文档正文，让「搜索正文内容」可用（默认关闭）。
              待提取的文件可能多达数十万个、总体积数十 GB，开启后首次会持续较长时间读盘。
              开发与构建目录（node_modules、target、dist 等）已自动跳过，只按文件名索引。
              关闭后仍可在「内容」页手动启动一次。
            </p>
          </div>
          <input
            type="checkbox"
            checked={autoFulltext}
            onChange={(e) => {
              setAutoFulltext(e.target.checked);
              setSetting("ui.autoFulltext", e.target.checked).catch(() => undefined);
            }}
          />
        </div>
      </section>

      <section className="card">
        <div className="card-head">
          <h3>退出</h3>
        </div>
        <div className="toggle-row">
          <div>
            <b>退出 FindFlow</b>
            <p className="muted">完全结束进程；关闭窗口只是隐藏到后台，不会退出</p>
          </div>
          <button
            className="btn-danger-text"
            onClick={() => appExit().catch((e) => notice(String(e)))}
          >
            退出
          </button>
        </div>
      </section>
    </div>
  );
}
