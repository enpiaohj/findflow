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
} from "../../api";
import { getThemePref, setThemePref, type ThemePref } from "../../theme";

/** 外观：主题与显示密度。 */
export function AppearanceSettingsPage({ notice: _notice }: PageCtx) {
  const [density, setDensity] = useState("compact");
  const [theme, setTheme] = useState<ThemePref>(getThemePref());

  useEffect(() => {
    getSetting<string>("ui.density").then((v) => v && setDensity(v)).catch(() => undefined);
  }, []);

  return (
    <div className="page">
      <h2>外观</h2>
      <p className="page-desc">主题与搜索窗列表的显示密度。</p>

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
    </div>
  );
}

/** 快捷键：全局呼出。 */
export function HotkeyPage({ notice }: PageCtx) {
  const [hotkey, setHotkey] = useState("Alt+Q");
  const [hotkeyCommitted, setHotkeyCommitted] = useState("");

  useEffect(() => {
    getSetting<string>("ui.hotkeyActive")
      .then((v) => {
        if (v) {
          setHotkey(v);
          setHotkeyCommitted(v);
        }
      })
      .catch(() => undefined);
  }, []);

  return (
    <div className="page">
      <h2>快捷键</h2>
      <p className="page-desc">全局呼出与隐藏搜索窗的快捷键。</p>

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
            <p className="muted">基础搜索完全本地完成；不上传文件名、路径、内容或搜索词</p>
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

export function AboutPage({ notice: _notice }: PageCtx) {
  const [version, setVersion] = useState("");

  useEffect(() => {
    // 版本号从应用元数据读取，避免写死后与实际版本不一致
    getVersion()
      .then(setVersion)
      .catch(() => undefined);
  }, []);

  return (
    <div className="page">
      <h2>关于</h2>
      <p className="page-desc">FindFlow 的版本与项目信息。</p>
      <section className="card">
        <div className="about-head">
          <b className="about-name">FindFlow</b>
          <span className="muted">Windows 本地优先的极速文件搜索工具</span>
        </div>
        <div className="about-row">
          <span className="about-label">版本</span>
          <span>{version ? `v${version}` : "读取中…"}</span>
        </div>
        <div className="about-row">
          <span className="about-label">开发者</span>
          <a href="https://github.com/enpiaohj" target="_blank" rel="noreferrer">
            enpiaohj（GitHub）
          </a>
        </div>
        <div className="about-row">
          <span className="about-label">源码仓库</span>
          <a href="https://github.com/enpiaohj/findflow" target="_blank" rel="noreferrer">
            https://github.com/enpiaohj/findflow
          </a>
        </div>
      </section>
      <section className="card">
        <div className="card-head">
          <h3>隐私与数据</h3>
        </div>
        <p className="muted">
          FindFlow 不收集遥测、崩溃报告或使用统计，不向任何第三方发送数据。搜索索引、设置与搜索历史（如开启）均仅保存在本机，
          可在「隐私与安全」中关闭或清空搜索历史。
        </p>
        <p className="muted">
          唯一的外部请求来自可选的「自然语言搜索」：仅在配置 DeepSeek API Key 并主动使用该功能时，
          把查询文本与字段定义发送给 DeepSeek，仅返回查询条件，不发送任何本地文件数据。
        </p>
      </section>
    </div>
  );
}
