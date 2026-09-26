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
  fulltextStart,
  ocrStart,
  storageUsage,
  setDataDir,
  relaunchApp,
  formatBytes,
  diagnosticsHealth,
  type ScopeInfo,
} from "../../api";

interface TrialScanResultLike {
  path: string;
  filesEst: number;
  bytesEst: number;
  sample: string[];
}

export default function IndexPage({ notice }: PageCtx) {
  const { confirm, confirmEl } = useConfirm();
  const [scopes, setScopes] = useState<ScopeInfo[]>([]);
  const [health, setHealth] = useState<{ totalFiles: number; fulltextDone: number; ocrDone: number } | null>(null);
  const [trial, setTrial] = useState<TrialScanResultLike | null>(null);
  const [busy, setBusy] = useState(false);
  const [usage, setUsage] = useState<{ dataDir: string; dbBytes: number; indexBytes: number } | null>(null);
  const [excludeEdit, setExcludeEdit] = useState<{
    scopeId: number;
    dirs: string;
    exts: string;
  } | null>(null);

  const reload = () => {
    scopesList().then(setScopes).catch(() => undefined);
    diagnosticsHealth().then(setHealth).catch(() => undefined);
    storageUsage().then(setUsage).catch(() => undefined);
  };
  useEffect(reload, []);
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
      <div className="health-banner">
        <span className="health-ok">✓</span>
        <span>索引正常</span>
        <span className="health-sep" />
        <span>已索引 {health?.totalFiles ?? 0} 个文件</span>
        <span className="health-sep" />
        <span>全文 {health?.fulltextDone ?? 0}</span>
        <span className="health-sep" />
        <span>OCR {health?.ocrDone ?? 0}</span>
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
                {s.filesCount} 个文件 · {s.state}
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
                  try {
                    const o = JSON.parse(s.optionsJson || '{}');
                    dirs = (o.exclude_dirs ?? []).join(', ');
                    exts = (o.exclude_exts ?? []).join(', ');
                  } catch {
                    /* 忽略解析失败 */
                  }
                  setExcludeEdit({ scopeId: s.scopeId, dirs, exts });
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
                        include_hidden: false,
                      },
                    })
                  )
                  .then(() => {
                    notice('排除规则已保存，重新扫描后生效');
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
