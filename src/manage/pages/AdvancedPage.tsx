import { useEffect, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import type { PageCtx } from "../ManageBody";
import {
  diagnosticsFailures,
  diagnosticsExport,
  diagnosticsIntegrity,
  logDirGet,
} from "../../api";

/** 高级：索引诊断与日志。 */
export default function AdvancedPage({ notice }: PageCtx) {
  const [failures, setFailures] = useState<{ code: string | null; count: number }[]>([]);
  const [integrity, setIntegrity] = useState<string | null>(null);
  const [logDir, setLogDir] = useState<string | null>(null);

  useEffect(() => {
    diagnosticsFailures().then(setFailures).catch(() => undefined);
    logDirGet().then(setLogDir).catch(() => undefined);
  }, []);

  return (
    <div className="page">
      <h2>高级</h2>
      <p className="page-desc">索引诊断、日志与数据库完整性检查。</p>

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
