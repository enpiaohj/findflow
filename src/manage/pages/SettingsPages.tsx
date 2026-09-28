import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import type { PageCtx } from "../ManageBody";
import {
  getSetting,
  setSetting,
  llmHasKey,
  llmSetKey,
  llmClearKey,
  hotkeyApply,
  autostartIsEnabled,
  autostartEnable,
  autostartDisable,
} from "../../api";
import { getThemePref, setThemePref, type ThemePref } from "../../theme";

export function AppearancePage({ notice }: PageCtx) {
  const [density, setDensity] = useState("compact");
  const [hotkey, setHotkey] = useState("Alt+Q");
  const [hotkeyCommitted, setHotkeyCommitted] = useState("");
  const [blurHide, setBlurHide] = useState(true);
  const [clearOnHide, setClearOnHide] = useState(false);
  const [theme, setTheme] = useState<ThemePref>(getThemePref());
  const [autostart, setAutostart] = useState(false);

  useEffect(() => {
    getSetting<string>("ui.density").then((v) => v && setDensity(v)).catch(() => undefined);
    getSetting<string>("ui.hotkeyActive")
      .then((v) => {
        if (v) {
          setHotkey(v);
          setHotkeyCommitted(v);
        }
      })
      .catch(() => undefined);
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
      <h2>外观与快捷键</h2>
      <p className="page-desc">主题、显示密度、全局快捷键与应用启动行为。</p>

      <section className="card">
        <div className="card-head">
          <h3>外观</h3>
        </div>
        <div className="toggle-row">
          <div>
            <b>主题</b>
            <p className="muted">跟随系统时随 Windows 深色 / 浅色设置自动切换</p>
          </div>
          <div className="segmented" role="radiogroup" aria-label="主题">
            {(
              [
                ["system", "跟随系统"],
                ["light", "浅色"],
                ["dark", "深色"],
              ] as const
            ).map(([id, label]) => (
              <button
                key={id}
                role="radio"
                aria-checked={theme === id}
                className={theme === id ? "active" : ""}
                onClick={() => {
                  setTheme(id);
                  setThemePref(id);
                }}
              >
                {label}
              </button>
            ))}
          </div>
        </div>
        <div className="toggle-row">
          <div>
            <b>默认显示密度</b>
            <p className="muted">搜索窗默认行高（可在搜索窗内随时切换并自动记忆）</p>
          </div>
          <select
            value={density}
            onChange={(e) => {
              setDensity(e.target.value);
              setSetting("ui.density", e.target.value).catch(() => undefined);
            }}
          >
            <option value="compact">紧凑</option>
            <option value="comfortable">舒适</option>
            <option value="large">大图标</option>
          </select>
        </div>
      </section>

      <section className="card">
        <div className="card-head">
          <h3>快捷键</h3>
        </div>
        <div className="toggle-row">
          <div>
            <b>全局快捷键</b>
            <p className="muted">呼出/隐藏搜索窗，保存后立即生效；与 PowerToys、uTools 等冲突时请换其他组合</p>
          </div>
          <input
            value={hotkey}
            onChange={(e) => setHotkey(e.target.value)}
            onBlur={() => {
              if (hotkey === hotkeyCommitted) return;
              hotkeyApply(hotkey)
                .then(() => {
                  setHotkeyCommitted(hotkey);
                  setSetting("ui.hotkey", hotkey).catch(() => undefined);
                  notice("快捷键已生效");
                })
                .catch((e) => {
                  notice(String(e));
                  setHotkey(hotkeyCommitted);
                });
            }}
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
    </div>
  );
}

export function PrivacyPage({ notice }: PageCtx) {
  const [hasKey, setHasKey] = useState(false);
  const [keyInput, setKeyInput] = useState("");

  useEffect(() => {
    llmHasKey().then(setHasKey).catch(() => undefined);
  }, []);

  return (
    <div className="page">
      <h2>隐私与安全</h2>
      <p className="page-desc">FindFlow 默认完全本地运行，你的文件内容不会离开这台电脑。</p>
      <section className="card">
        <div className="toggle-row">
          <div>
            <b>本地优先</b>
            <p className="muted">基础搜索完全本地完成；不上传文件名、路径、内容或搜索词；搜索词不落盘</p>
          </div>
        </div>
        <div className="toggle-row">
          <div>
            <b>删除策略</b>
            <p className="muted">默认删除进入系统回收站；永久删除需要独立确认令牌，且不提供全局快捷键</p>
          </div>
        </div>
        <div className="toggle-row">
          <div>
            <b>日志脱敏</b>
            <p className="muted">诊断日志保留 14 天并对路径做缩短处理，不含文件正文</p>
          </div>
        </div>
      </section>
      <section className="card">
        <div className="card-head">
          <h3>DeepSeek API Key（可选，用于自然语言搜索）</h3>
        </div>
        <p className="muted">
          Key 使用 Windows DPAPI（当前用户级）加密存储在本机，不写入代码、配置文件或仓库。
          启用后，自然语言搜索会把「查询文本 + 字段定义」发送给 DeepSeek，仅返回查询条件，不发送任何本地文件数据。
        </p>
        <div className="tag-create">
          <input
            type="password"
            value={keyInput}
            placeholder="sk-..."
            onChange={(e) => setKeyInput(e.target.value)}
          />
          <button
            onClick={() => {
              if (!keyInput.trim()) return;
              llmSetKey(keyInput)
                .then(() => {
                  setHasKey(true);
                  setKeyInput("");
                  notice("API Key 已加密保存");
                })
                .catch((e) => notice(String(e)));
            }}
          >
            保存 Key
          </button>
          {hasKey ? (
            <button
              className="btn-danger-text"
              onClick={() =>
                llmClearKey()
                  .then(() => {
                    setHasKey(false);
                    notice("已清除 API Key");
                  })
                  .catch((e) => notice(String(e)))
              }
            >
              清除已保存的 Key
            </button>
          ) : null}
        </div>
        <p className="muted">{hasKey ? "✓ 已配置 API Key" : "未配置（自然语言离线规则仍可用）"}</p>
      </section>
    </div>
  );
}

export function UpdatePage({ notice: _notice }: PageCtx) {
  const [version, setVersion] = useState("");

  useEffect(() => {
    // 版本号从应用元数据读取，避免写死后与实际版本不一致
    getVersion()
      .then(setVersion)
      .catch(() => undefined);
  }, []);

  return (
    <div className="page">
      <h2>更新</h2>
      <p className="page-desc">查看 FindFlow 的版本信息。</p>
      <section className="card">
        <div className="toggle-row">
          <div>
            <b>当前版本</b>
            <p className="muted">{version ? `v${version}` : "读取中…"}</p>
          </div>
        </div>
      </section>
    </div>
  );
}
