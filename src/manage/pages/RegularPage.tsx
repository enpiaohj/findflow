import { useEffect, useState } from "react";
import type { PageCtx } from "../ManageBody";
import { getSetting, setSetting, autostartIsEnabled, autostartEnable, autostartDisable } from "../../api";

/** 常规：应用启动与窗口行为。 */
export default function RegularPage({ notice }: PageCtx) {
  const [blurHide, setBlurHide] = useState(true);
  const [clearOnHide, setClearOnHide] = useState(false);
  const [autostart, setAutostart] = useState(false);

  useEffect(() => {
    getSetting<boolean>("ui.blurHide").then((v) => v !== null && setBlurHide(v)).catch(() => undefined);
    getSetting<boolean>("ui.clearOnHide").then((v) => setClearOnHide(!!v)).catch(() => undefined);
    autostartIsEnabled().then(setAutostart).catch(() => undefined);
  }, []);

  const changeAutostart = (checked: boolean) => {
    (checked ? autostartEnable() : autostartDisable())
      .then(() => setAutostart(checked))
      .catch((e) => notice(String(e)));
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
            <p className="muted">开启后窗口隐藏即清空搜索词（避免敏感词残留）；默认关闭，打开文件返回后保留上次搜索</p>
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
      </section>
    </div>
  );
}
