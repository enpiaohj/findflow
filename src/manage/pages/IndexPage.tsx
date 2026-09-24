import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import type { PageCtx } from "../ManageApp";
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
  const [scopes, setScopes] = useState<ScopeInfo[]>([]);
  const [health, setHealth] = useState<{ totalFiles: number; fulltextDone: number; ocrDone: number } | null>(null);
  const [trial, setTrial] = useState<TrialScanResultLike | null>(null);
  const [busy, setBusy] = useState(false);

  const reload = () => {
    scopesList().then(setScopes).catch((e) => notice(String(e)));
    diagnosticsHealth().then(setHealth).catch(() => undefined);
  };
  useEffect(reload, []);

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
      await scopeAdd(trial.path);
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
                className="btn-danger-text"
                onClick={() => {
                  if (window.confirm(`移除索引范围 ${s.path}？（同时删除该范围索引）`)) {
                    scopeRemove(s.scopeId, true).then(reload).catch((e) => notice(String(e)));
                  }
                }}
              >
                移除
              </button>
            </div>
          </div>
        ))}
      </section>

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
