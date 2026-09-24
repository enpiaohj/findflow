import { useEffect, useState } from "react";
import type { PageCtx } from "../ManageApp";
import {
  getSetting,
  setSetting,
  llmHasKey,
  llmSetKey,
  llmClearKey,
  updaterCheck,
  updaterInstall,
} from "../../api";

export function AppearancePage({ notice }: PageCtx) {
  const [density, setDensity] = useState("compact");
  const [hotkey, setHotkey] = useState("Alt+Space");
  const [blurHide, setBlurHide] = useState(true);

  useEffect(() => {
    getSetting<string>("ui.density").then((v) => v && setDensity(v)).catch(() => undefined);
    getSetting<string>("ui.hotkey").then((v) => v && setHotkey(v)).catch(() => undefined);
    getSetting<boolean>("ui.blurHide").then((v) => v !== null && setBlurHide(v)).catch(() => undefined);
  }, []);

  return (
    <div className="page">
      <h2>外观与快捷键</h2>
      <section className="card">
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
        <div className="toggle-row">
          <div>
            <b>全局快捷键</b>
            <p className="muted">呼出/隐藏搜索窗。修改后重启应用生效；与 PowerToys 等冲突时请改用其他组合</p>
          </div>
          <input
            value={hotkey}
            onChange={(e) => setHotkey(e.target.value)}
            onBlur={() => setSetting("ui.hotkey", hotkey).then(() => notice("快捷键已保存，重启应用后生效")).catch(() => undefined)}
          />
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

export function UpdatePage({ notice }: PageCtx) {
  const [status, setStatus] = useState<string>("点击检查更新");
  const [busy, setBusy] = useState(false);

  const check = () => {
    setBusy(true);
    updaterCheck()
      .then((r) =>
        setStatus(
          r.available ? `发现新版本 v${r.version}，可下载安装` : "当前已是最新版本"
        )
      )
      .catch((e) => setStatus(String(e)))
      .finally(() => setBusy(false));
  };

  return (
    <div className="page">
      <h2>更新</h2>
      <section className="card">
        <div className="toggle-row">
          <div>
            <b>当前版本</b>
            <p className="muted">v0.2.0 · 更新包经 minisign 签名校验，迁移失败自动回滚</p>
          </div>
        </div>
        <div className="trial-actions">
          <span>{status}</span>
          <button disabled={busy} onClick={check}>
            {busy ? "检查中…" : "检查更新"}
          </button>
          <button
            className="btn btn-primary"
            disabled={busy}
            onClick={() =>
              updaterInstall()
                .then(() => notice("更新完成，应用将重启"))
                .catch((e) => notice(String(e)))
            }
          >
            下载并安装更新
          </button>
        </div>
      </section>
    </div>
  );
}
