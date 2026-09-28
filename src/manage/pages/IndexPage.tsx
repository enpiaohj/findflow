import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { PageCtx } from "../ManageBody";
import { useConfirm } from "../../components/ConfirmDialog";
import {
  getSetting,
  setSetting,
  scopesList,
  scopeAdd,
  scopeRemove,
  scopeSetEnabled,
  scopeTrialScan,
  scanScope,
  recalibrateScope,
  rebuildScope,
  fulltextStart,
  ocrStart,
  storageUsage,
  setDataDir,
  relaunchApp,
  formatBytes,
  diagnosticsHealth,
  type ScopeInfo,
} from "../../api";
import {
  DEFAULT_START_VIEW,
  LEGACY_START_LOCATION_KEY,
  START_VIEW_KEY,
  resolveStartView,
  type StartViewSetting,
} from "../../startView";

interface TrialScanResultLike {
  path: string;
  filesEst: number;
  bytesEst: number;
  sample: string[];
}

/** 范围状态的中文显示。 */
const SCOPE_STATES: Record<string, string> = {
  idle: "已就绪",
  scanning: "扫描中",
  offline: "已停用",
};

export default function IndexPage({ notice }: PageCtx) {
  const { confirm, confirmEl } = useConfirm();
  const [scopes, setScopes] = useState<ScopeInfo[]>([]);
  const [health, setHealth] = useState<{
    totalFiles: number;
    fulltextDone: number;
    ocrDone: number;
    outboxPending: number;
  } | null>(null);
  const [trial, setTrial] = useState<TrialScanResultLike | null>(null);
  const [busy, setBusy] = useState(false);
  const [usage, setUsage] = useState<{ dataDir: string; dbBytes: number; indexBytes: number } | null>(null);
  const [excludeEdit, setExcludeEdit] = useState<{
    scopeId: number;
    dirs: string;
    exts: string;
    includeHidden: boolean;
  } | null>(null);
  const [startView, setStartViewState] = useState<StartViewSetting>({
    ...DEFAULT_START_VIEW,
  });
  const [startViewLoaded, setStartViewLoaded] = useState(false);

  const reload = () => {
    scopesList().then(setScopes).catch(() => undefined);
    diagnosticsHealth().then(setHealth).catch(() => undefined);
    storageUsage().then(setUsage).catch(() => undefined);
  };
  useEffect(() => {
    let cancelled = false;

    Promise.all([
      scopesList(),
      getSetting<unknown>(START_VIEW_KEY),
      getSetting<unknown>(LEGACY_START_LOCATION_KEY),
    ])
      .then(([nextScopes, currentValue, legacyValue]) => {
        if (cancelled) return;
        const resolved = resolveStartView(currentValue, legacyValue, nextScopes);
        setScopes(nextScopes);
        setStartViewState(resolved.setting);
        setStartViewLoaded(true);
        if (resolved.needsPersist) {
          setSetting(START_VIEW_KEY, resolved.setting).catch((error) =>
            notice("启动列表设置迁移失败：" + String(error)),
          );
        }
      })
      .catch((error) => {
        if (!cancelled) notice("读取启动列表设置失败：" + String(error));
      });

    diagnosticsHealth()
      .then((nextHealth) => {
        if (!cancelled) setHealth(nextHealth);
      })
      .catch(() => undefined);
    storageUsage()
      .then((nextUsage) => {
        if (!cancelled) setUsage(nextUsage);
      })
      .catch(() => undefined);

    return () => {
      cancelled = true;
    };
  }, [notice]);
  useEffect(() => {
    if (!startViewLoaded) return;
    const resolved = resolveStartView(startView, null, scopes);
    if (!resolved.needsPersist) return;

    setStartViewState(resolved.setting);
    setSetting(START_VIEW_KEY, resolved.setting).catch((error) =>
      notice("默认索引位置更新失败：" + String(error)),
    );
  }, [notice, scopes, startView, startViewLoaded]);

  const saveStartView = async (next: StartViewSetting) => {
    try {
      await setSetting(START_VIEW_KEY, next);
      setStartViewState(next);
    } catch (error) {
      notice("保存启动列表设置失败：" + String(error));
    }
  };

  const enabledScopes = scopes.filter((scope) => scope.enabled);
  // 渲染期直接派生显示用的启动列表设置：读取完成前不渲染，位置失效时无需等回退 effect 提交，
  // 避免「索引内容」按钮闪现与下拉短暂空白（显示与数据始终一致）
  const resolvedStartView = startViewLoaded
    ? resolveStartView(startView, null, scopes).setting
    : null;
  // 范围状态/数量轮询刷新，扫描完成后列表自动更新
  useEffect(() => {
    const t = window.setInterval(() => {
      scopesList().then(setScopes).catch(() => undefined);
      diagnosticsHealth().then(setHealth).catch(() => undefined);
    }, 3000);
    return () => window.clearInterval(t);
  }, []);

  const addFolder = async () => {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked !== "string") return;
    setBusy(true);
    try {
      const result = await scopeTrialScan(picked);
      setTrial({ path: picked, ...result });
    } catch (e) {
      notice(String(e));
    } finally {
      setBusy(false);
    }
  };

  const confirmAdd = async () => {
    if (!trial) return;
    try {
      await scopeAdd(trial.path, undefined, trial.filesEst);
      notice(`已加入索引并开始扫描：${trial.path}`);
      setTrial(null);
      reload();
    } catch (e) {
      notice(String(e));
    }
  };

  return (
    <div className="page">
      <h2>索引与搜索</h2>
      <p className="page-desc">管理 FindFlow 的文件索引范围与搜索行为，让搜索更快、更准确。</p>
      <div className={"health-banner" + (health && health.outboxPending > 0 ? " busy" : "")}>
        <span className="health-ok">{health && health.outboxPending > 0 ? "↻" : "✓"}</span>
        <b>
          {health && health.outboxPending > 0
            ? `正在更新索引（剩余 ${health.outboxPending.toLocaleString()} 项）`
            : "索引正常"}
        </b>
        <span className="health-sep" />
        <span>已索引 {(health?.totalFiles ?? 0).toLocaleString()} 个文件</span>
        <span className="health-sep" />
        <span>全文 {(health?.fulltextDone ?? 0).toLocaleString()}</span>
        <span className="health-sep" />
        <span>OCR {(health?.ocrDone ?? 0).toLocaleString()}</span>
      </div>

      <section className="card">
        <div className="card-head">
          <h3>索引范围</h3>
          <button className="btn btn-primary" disabled={busy} onClick={() => void addFolder()}>
            ＋ 添加文件夹
          </button>
        </div>
        {scopes.length === 0 ? <p className="muted">尚未配置索引范围。建议从常用资料目录开始。</p> : null}
        {scopes.map((s) => (
          <div key={s.scopeId} className="scope-row">
            <label className="scope-enable">
              <input
                type="checkbox"
                checked={s.enabled}
                onChange={(e) =>
                  scopeSetEnabled(s.scopeId, e.target.checked)
                    .then(reload)
                    .catch((err) => notice(String(err)))
                }
              />
            </label>
            <div className="scope-info">
              <div className="scope-path">{s.path}</div>
              <div className="scope-meta">
                {s.filesCount.toLocaleString()} 个文件 · {SCOPE_STATES[s.state] ?? s.state}
                {s.baselineAt ? ` · 基线 ${new Date(s.baselineAt).toLocaleString()}` : ""}
              </div>
            </div>
            <div className="scope-actions">
              <button onClick={() => scanScope(s.scopeId).then(reload).catch((e) => notice(String(e)))}>
                重新扫描
              </button>
              <button onClick={() => recalibrateScope(s.scopeId).then(reload).catch((e) => notice(String(e)))}>
                校准
              </button>
              <button onClick={() => rebuildScope(s.scopeId).then(reload).catch((e) => notice(String(e)))}>
                重建
              </button>
              <button
                onClick={() => {
                  let dirs = '';
                  let exts = '';
                  let includeHidden = false;
                  try {
                    const o = JSON.parse(s.optionsJson || '{}');
                    dirs = (o.exclude_dirs ?? []).join(', ');
                    exts = (o.exclude_exts ?? []).join(', ');
                    includeHidden = !!o.include_hidden;
                  } catch {
                    /* 忽略解析失败 */
                  }
                  setExcludeEdit({ scopeId: s.scopeId, dirs, exts, includeHidden });
                }}
              >
                排除规则
              </button>
              <button
                className="btn-danger-text"
                onClick={async () => {
                  if (
                    !(await confirm(
                      `移除索引范围 ${s.path}？`,
                      "将同时删除该范围的索引数据，操作不可撤销。"
                    ))
                  )
                    return;
                  scopeRemove(s.scopeId, true)
                    .then(reload)
                    .catch((e) => notice(String(e)));
                }}
              >
                移除
              </button>
            </div>
          </div>
        ))}
      </section>

      <section className="card">
        <div className="card-head">
          <h3>启动列表</h3>
        </div>
        {resolvedStartView ? (
          <>
            <div className="toggle-row">
              <div>
                <b>打开 FindFlow 时显示</b>
                <p className="muted">仅控制搜索框为空时的列表；输入关键词后始终搜索全部索引位置</p>
              </div>
              <div className="segmented">
                <button
                  className={resolvedStartView.mode === "index" ? "active" : ""}
                  onClick={() =>
                    void saveStartView(
                      resolveStartView({ mode: "index", scopeId: resolvedStartView.scopeId }, null, scopes)
                        .setting,
                    )
                  }
                >
                  索引内容
                </button>
                <button
                  className={resolvedStartView.mode === "large" ? "active" : ""}
                  onClick={() => void saveStartView({ mode: "large", scopeId: null })}
                >
                  大文件
                </button>
              </div>
            </div>
            {resolvedStartView.mode === "index" ? (
              <div className="toggle-row">
                <div>
                  <b>默认索引位置</b>
                  <p className="muted">空搜索时列出该位置下的全部索引内容，并按名称排序</p>
                </div>
                <select
                  value={resolvedStartView.scopeId ?? ""}
                  onChange={(event) =>
                    void saveStartView({
                      mode: "index",
                      scopeId: Number(event.target.value),
                    })
                  }
                  disabled={enabledScopes.length === 0}
                >
                  {enabledScopes.length === 0 ? <option value="">暂无启用的索引位置</option> : null}
                  {enabledScopes.map((scope) => (
                    <option key={scope.scopeId} value={scope.scopeId}>
                      {scope.path}
                    </option>
                  ))}
                </select>
              </div>
            ) : (
              <div className="toggle-row">
                <div>
                  <b>大文件范围</b>
                  <p className="muted">跨全部索引位置，并按文件大小降序显示</p>
                </div>
                <span className="toggle-on">全部位置</span>
              </div>
            )}
          </>
        ) : (
          <div className="toggle-row">
            <div>
              <b>打开 FindFlow 时显示</b>
              <p className="muted">正在读取启动列表设置…</p>
            </div>
          </div>
        )}
      </section>

      {excludeEdit ? (
        <section className="card trial-card">
          <div className="card-head">
            <h3>排除规则（范围 #{excludeEdit.scopeId}）</h3>
          </div>
          <div className="rule-grid">
            <label>
              排除目录（逗号分隔，匹配路径段）
              <input
                value={excludeEdit.dirs}
                placeholder="node_modules, .git, temp"
                onChange={(e) => setExcludeEdit({ ...excludeEdit, dirs: e.target.value })}
              />
            </label>
            <label>
              排除扩展名（逗号分隔）
              <input
                value={excludeEdit.exts}
                placeholder="tmp, log, bak"
                onChange={(e) => setExcludeEdit({ ...excludeEdit, exts: e.target.value })}
              />
            </label>
          </div>
          <div className="trial-actions">
            <span className="muted">常用预设：</span>
            {['node_modules', '.git', 'target', 'dist', 'build', '.venv'].map((p) => (
              <button
                key={p}
                onClick={() =>
                  setExcludeEdit((prev) =>
                    prev
                      ? {
                          ...prev,
                          dirs: prev.dirs
                            ? prev.dirs + ', ' + p
                            : p,
                        }
                      : prev,
                  )
                }
              >
                + {p}
              </button>
            ))}
          </div>
          <div className="trial-actions">
            <button onClick={() => setExcludeEdit(null)}>取消</button>
            <button
              className="btn btn-primary"
              onClick={() => {
                const dirs = excludeEdit.dirs
                  .split(/[,,]/)
                  .map((x) => x.trim())
                  .filter(Boolean);
                const exts = excludeEdit.exts
                  .split(/[,,]/)
                  .map((x) => x.trim().replace(/^\./, ''))
                  .filter(Boolean);
                scopeSetEnabled(excludeEdit.scopeId, true)
                  .then(() =>
                    invoke('scope_update_options', {
                      scopeId: excludeEdit.scopeId,
                      options: {
                        exclude_dirs: dirs,
                        exclude_exts: exts,
                        include_hidden: excludeEdit.includeHidden,
                      },
                    })
                  )
                  // 校准会移除新排除的条目并补入新纳入的条目
                  .then(() => recalibrateScope(excludeEdit.scopeId))
                  .then(() => {
                    notice('排除规则已保存，正在后台校准该范围');
                    setExcludeEdit(null);
                    reload();
                  })
                  .catch((e) => notice(String(e)));
              }}
            >
              保存规则并重扫
            </button>
          </div>
        </section>
      ) : null}

      {trial ? (
        <section className="card trial-card">
          <h3>试扫确认</h3>
          <p>
            <b>{trial.path}</b>
          </p>
          <p>
            预计 {trial.filesEst.toLocaleString()} 个文件 · 所选文件合计约 {Math.max(1, Math.round(trial.bytesEst / 1024 / 1024))} MB
          </p>
          <ul className="trial-sample">
            {trial.sample.slice(0, 5).map((s) => (
              <li key={s}>{s}</li>
            ))}
          </ul>
          <div className="trial-actions">
            <button onClick={() => setTrial(null)}>取消</button>
            <button className="btn btn-primary" onClick={() => void confirmAdd()}>
              确认并开始后台扫描
            </button>
          </div>
        </section>
      ) : null}

      <section className="card">
        <div className="card-head">
          <h3>存储位置</h3>
          <button
            onClick={async () => {
              const d = await open({ directory: true, multiple: false });
              if (typeof d === 'string') {
                try {
                  await setDataDir(d);
                  notice('存储位置已保存，重启应用后自动迁移数据');
                } catch (e) {
                  notice(String(e));
                }
              }
            }}
          >
            更改存储位置
          </button>
        </div>
        <p className="muted">{usage?.dataDir ?? '读取中…'}</p>
        <p className="muted">数据库 {formatBytes(usage?.dbBytes ?? 0)} · 倒排索引 {formatBytes(usage?.indexBytes ?? 0)}</p>
        <p className="muted">更改后重启应用：数据库自动迁移到新位置，索引在新位置重建。</p>
        <button
          className="btn-primary"
          onClick={() => relaunchApp().catch((err: unknown) => notice(String(err)))}
        >
          立即重启并迁移
        </button>
      </section>

      <section className="card">
        <div className="card-head">
          <h3>索引内容</h3>
        </div>
        <div className="toggle-row">
          <div>
            <b>文件名与元数据</b>
            <p className="muted">文件名、类型、大小、修改时间等基本信息（始终启用，推荐）</p>
          </div>
          <span className="toggle-on">启用</span>
        </div>
        <div className="toggle-row">
          <div>
            <b>文档全文</b>
            <p className="muted">解析 Office、PDF、文本与代码文件内容，供全文搜索与命中片段</p>
          </div>
          <button onClick={() => fulltextStart().then(() => notice("全文提取任务已开始，可在「任务与诊断」查看进度")).catch((e) => notice(String(e)))}>
            启动提取
          </button>
        </div>
        <div className="toggle-row">
          <div>
            <b>图片 OCR</b>
            <p className="muted">识别图片中的文字（使用系统 OCR，需要已安装对应语言包）</p>
          </div>
          <button onClick={() => ocrStart().then(() => notice("OCR 任务已开始")).catch((e) => notice(String(e)))}>
            启动 OCR
          </button>
        </div>
      </section>

      {confirmEl}
      <section className="card">
        <div className="card-head">
          <h3>索引操作</h3>
        </div>
        <div className="toggle-row">
          <div>
            <b>暂停 / 恢复监听</b>
            <p className="muted">在「索引范围」列表中取消勾选即可暂停对应范围的监听与更新</p>
          </div>
        </div>
        <div className="toggle-row">
          <div>
            <b>重建指定范围</b>
            <p className="muted">范围行内点击「重建」，会清空该范围索引后重新扫描</p>
          </div>
        </div>
      </section>
    </div>
  );
}
