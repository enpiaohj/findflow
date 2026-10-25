import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { PageCtx } from "../ManageBody";
import { useConfirm } from "../../components/ConfirmDialog";
import {
  scopesList,
  scopeAdd,
  scopeRemove,
  scopeSetEnabled,
  scopeTrialScan,
  scanScope,
  recalibrateScope,
  rebuildScope,
  storageUsage,
  cleanupLegacyData,
  setDataDir,
  relaunchApp,
  formatBytes,
  diagnosticsHealth,
  type ScopeInfo,
  type StorageUsage,
} from "../../api";

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
  const [usage, setUsage] = useState<StorageUsage | null>(null);
  const [excludeEdit, setExcludeEdit] = useState<{
    scopeId: number;
    dirs: string;
    exts: string;
    includeHidden: boolean;
  } | null>(null);

  const reload = () => {
    scopesList().then(setScopes).catch(() => undefined);
    diagnosticsHealth().then(setHealth).catch(() => undefined);
    storageUsage().then(setUsage).catch(() => undefined);
  };
  useEffect(() => {
    let cancelled = false;

    scopesList()
      .then((nextScopes) => {
        if (!cancelled) setScopes(nextScopes);
      })
      .catch((error) => {
        if (!cancelled) notice("读取索引范围失败：" + String(error));
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
      <h2>索引</h2>
      <p className="page-desc">管理索引位置、排除规则与数据库存储位置。</p>
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
                {s.filesCount.toLocaleString()} 个文件 · {s.enabled ? (SCOPE_STATES[s.state] ?? s.state) : "已关闭，不参与搜索"}
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
                if (d === usage?.dataDir) {
                  notice('新位置与当前存储位置相同，无需迁移');
                  return;
                }
                try {
                  await setDataDir(d);
                  notice('迁移计划已保存；重启后复制并校验数据库，成功后切换。原数据库与索引保留');
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
        <p className="muted">请选择空目录。重启后生成包含 WAL 的数据库快照，校验成功才切换位置并重建索引；失败保留原数据库。原位置不会自动清理，请确认新位置工作正常后再处理。</p>
        <button
          className="btn-primary"
          onClick={() => relaunchApp().catch((err: unknown) => notice(String(err)))}
        >
          立即重启并迁移
        </button>
        {usage?.legacyDir ? (
          <div className="legacy-notice">
            <p>
              检测到旧数据目录仍有 <b>{formatBytes(usage.legacyBytes)}</b> 冗余（数据库与索引），
              位于 <code>{usage.legacyDir}</code>。
            </p>
            <p className="muted">清理不会影响当前索引与你的 API Key（密钥文件保留）。</p>
            <button
              onClick={async () => {
                if (!(await confirm(`清理旧数据目录中的 ${formatBytes(usage.legacyBytes)} 冗余？`, '仅删除旧数据库与旧索引，保留密钥与日志。'))) return;
                try {
                  const freed = await cleanupLegacyData();
                  notice(`已清理旧数据，释放 ${formatBytes(freed)}`);
                  storageUsage().then(setUsage).catch(() => undefined);
                } catch (e) {
                  notice(String(e));
                }
              }}
            >
              清理旧数据（释放 {formatBytes(usage.legacyBytes)}）
            </button>
          </div>
        ) : null}
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
