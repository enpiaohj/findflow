import { useEffect, useState } from "react";
import type { PageCtx } from "../ManageBody";
import {
  jobsList,
  jobPause,
  jobResume,
  jobCancel,
  jobRetryFailed,
  diagnosticsHealth,
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

const JOB_STATE_LABEL: Record<string, string> = {
  completed: "已完成",
  failed: "失败",
  cancelled: "已取消",
  interrupted: "已中断",
  running: "运行中",
  waiting: "等待中",
  paused: "已暂停",
};

/** 把任务结果 JSON 转为中文摘要（未知字段不展示，解析失败时原样返回）。 */
function summarizeResult(raw: string | null): string {
  if (!raw) return "—";
  let r: Record<string, unknown>;
  try {
    r = JSON.parse(raw);
  } catch {
    return raw;
  }
  const n = (k: string) => (typeof r[k] === "number" ? (r[k] as number).toLocaleString() : null);
  const parts: string[] = [];
  if (typeof r.error === "string") parts.push(`错误：${r.error}`);
  if (n("indexed") !== null) parts.push(`已索引 ${n("indexed")} 项`);
  if (n("extracted") !== null) parts.push(`已提取 ${n("extracted")} 个文件`);
  if (n("removed") !== null) parts.push(`清理 ${n("removed")} 条过期记录`);
  if (Array.isArray(r.groups)) parts.push(`找到 ${r.groups.length.toLocaleString()} 组重复文件`);
  if (r.failed && n("failed") !== "0") parts.push(`失败 ${n("failed")}`);
  if (r.errors && n("errors") !== "0") parts.push(`${n("errors")} 项无法访问`);
  return parts.length ? parts.join("，") : "—";
}

function formatJobTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const p = (x: number) => String(x).padStart(2, "0");
  return `${d.getFullYear()}/${p(d.getMonth() + 1)}/${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

export default function TasksPage({ notice }: PageCtx) {
  const [jobs, setJobs] = useState<JobInfo[]>([]);
  const [health, setHealth] = useState<IndexHealth | null>(null);

  const reload = () => {
    jobsList(50).then(setJobs).catch(() => undefined);
    diagnosticsHealth().then(setHealth).catch(() => undefined);
  };

  useEffect(() => {
    reload();
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
      <h2>任务</h2>
      <p className="page-desc">查看后台任务进度与索引健康状况。</p>
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
              <div
                className={
                  "progress-bar" +
                  (j.progress <= 0.01 ? " progress-indeterminate" : "")
                }
              >
                {j.progress <= 0.01 ? null : (
                  <div style={{ width: `${Math.round(j.progress * 100)}%` }} />
                )}
              </div>
              <span className="muted">
                {j.progress <= 0.01 ? (j.message ?? "进行中") : Math.round(j.progress * 100) + "%"}
              </span>
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
              <th>完成时间</th>
              <th>状态</th>
              <th>结果</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            {recent.map((j) => (
              <tr key={j.jobId}>
                <td>{JOB_KIND_LABEL[j.kind] ?? j.kind}</td>
                <td className="muted">{formatJobTime(j.updatedAt)}</td>
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
                    {JOB_STATE_LABEL[j.state] ?? j.state}
                  </span>
                </td>
                <td className="cell-path" title={j.resultJson ?? undefined}>
                  {summarizeResult(j.resultJson)}
                </td>
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
    </div>
  );
}
