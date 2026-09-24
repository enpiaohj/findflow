import { useEffect, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import type { PageCtx } from "../ManageBody";
import {
  jobsList,
  jobPause,
  jobResume,
  jobCancel,
  jobRetryFailed,
  diagnosticsHealth,
  diagnosticsFailures,
  diagnosticsExport,
  diagnosticsIntegrity,
  logDirGet,
  formatBytes,
  type JobInfo,
  type IndexHealth,
} from "../../api";

const JOB_KIND_LABEL: Record<string, string> = {
  initial_scan: "初始扫描",
  recalibrate: "校准扫描",
  fulltext: "全文提取",
  ocr: "图片 OCR",
  duplicates: "重复文件扫描",
};

export default function TasksPage({ notice }: PageCtx) {
  const [jobs, setJobs] = useState<JobInfo[]>([]);
  const [health, setHealth] = useState<IndexHealth | null>(null);
  const [failures, setFailures] = useState<{ code: string | null; count: number }[]>([]);
  const [integrity, setIntegrity] = useState<string | null>(null);
  const [logDir, setLogDir] = useState<string | null>(null);

  const reload = () => {
    jobsList(50).then(setJobs).catch(() => undefined);
    diagnosticsHealth().then(setHealth).catch(() => undefined);
    diagnosticsFailures().then(setFailures).catch(() => undefined);
  };

  useEffect(() => {
    reload();
    logDirGet().then(setLogDir).catch(() => undefined);
    const t = window.setInterval(reload, 3000);
    return () => window.clearInterval(t);
  }, []);

  const running = jobs.filter((j) => j.state === "running" || j.state === "waiting" || j.state === "paused");
  const recent = jobs.filter((j) => !running.includes(j)).slice(0, 8);

  const act = (p: Promise<void>, label: string) =>
    p.then(() => {
      notice(label);
      reload();
    }).catch((e) => notice(String(e)));

  return (
    <div className="page">
      <h2>任务与诊断</h2>
      <div className="health-banner">
        <span className="health-ok">✓</span>
        <span>系统状态</span>
        <span className="health-sep" />
        <span>索引 {health?.totalFiles ?? 0} 条</span>
        <span className="health-sep" />
        <span>数据库 {formatBytes(health?.dbSizeBytes ?? 0)}</span>
        <span className="health-sep" />
        <span>待索引事件 {health?.outboxPending ?? 0}</span>
      </div>

      <section className="card">
        <h3>正在运行（{running.length}）</h3>
        {running.length === 0 ? <p className="muted">没有正在运行的任务</p> : null}
        {running.map((j) => (
          <div key={j.jobId} className="job-row">
            <div className="job-info">
              <b>{JOB_KIND_LABEL[j.kind] ?? j.kind}</b>
              <span className="muted"> {j.message ?? j.state}</span>
            </div>
            <div className="progress-line">
              <div className="progress-bar">
                <div style={{ width: `${Math.round(j.progress * 100)}%` }} />
              </div>
              <span className="muted">{Math.round(j.progress * 100)}%</span>
            </div>
            <div className="scope-actions">
              {j.state === "paused" ? (
                <button onClick={() => act(jobResume(j.jobId), "任务已恢复")}>恢复</button>
              ) : (
                <button onClick={() => act(jobPause(j.jobId), "任务已暂停")}>暂停</button>
              )}
              <button onClick={() => act(jobCancel(j.jobId), "已请求取消")}>取消</button>
            </div>
          </div>
        ))}
      </section>

      <section className="card">
        <h3>最近任务</h3>
        <table className="data-table">
          <thead>
            <tr>
              <th>任务</th>
              <th>状态</th>
              <th>结果</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            {recent.map((j) => (
              <tr key={j.jobId}>
                <td>{JOB_KIND_LABEL[j.kind] ?? j.kind}</td>
                <td>
                  <span
                    className={
                      "status-badge " +
                      (j.state === "completed"
                        ? "badge-ok"
                        : j.state === "failed"
                          ? "badge-err"
                          : j.state === "cancelled" || j.state === "interrupted"
                            ? "badge-muted"
                            : "badge-warn")
                    }
                  >
                    {j.state}
                  </span>
                </td>
                <td className="muted cell-path">{j.resultJson ?? "—"}</td>
                <td>
                  {j.state === "failed" ? (
                    <button onClick={() => act(jobRetryFailed(j.jobId), "已重新入队")}>重试失败项</button>
                  ) : null}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      <section className="card">
        <h3>索引诊断</h3>
        <table className="data-table">
          <thead>
            <tr>
              <th>问题类型</th>
              <th className="col-right">数量</th>
            </tr>
          </thead>
          <tbody>
            {failures.length === 0 ? (
              <tr>
                <td className="muted">无</td>
                <td className="col-right muted">0</td>
              </tr>
            ) : null}
            {failures.map((f) => (
              <tr key={f.code ?? "unknown"}>
                <td>{f.code ?? "未知"}</td>
                <td className="col-right">{f.count}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <div className="trial-actions">
          <button
            onClick={async () => {
              const target = await save({
                defaultPath: "findflow-diagnostics.json",
                filters: [{ name: "JSON", extensions: ["json"] }],
              });
              if (target) {
                diagnosticsExport(target)
                  .then((t) => notice(`诊断包已导出：${t}`))
                  .catch((e) => notice(String(e)));
              }
            }}
          >
            导出诊断包
          </button>
          <button
            onClick={() =>
              diagnosticsIntegrity()
                .then(setIntegrity)
                .catch((e) => notice(String(e)))
            }
          >
            数据库完整性检查
          </button>
          {integrity ? <span className="muted">quick_check：{integrity}</span> : null}
        </div>
        {logDir ? <p className="muted">日志目录（保留 14 天）：{logDir}</p> : null}
      </section>
    </div>
  );
}
