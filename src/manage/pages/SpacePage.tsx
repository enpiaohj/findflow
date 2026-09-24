import { useEffect, useState } from "react";
import type { PageCtx } from "../ManageApp";
import {
  largeFiles,
  duplicatesStart,
  similarImages,
  jobsList,
  issueDeleteToken,
  actionRecycle,
  formatBytes,
  formatTime,
  type FileRow,
  type JobInfo,
} from "../../api";

interface DupGroup {
  hash: string;
  files: FileRow[];
  wasted: number;
}

export default function SpacePage({ notice }: PageCtx) {
  const [tab, setTab] = useState<"large" | "dup" | "similar">("large");
  const [large, setLarge] = useState<FileRow[]>([]);
  const [minSize, setMinSize] = useState(100 * 1024 * 1024);
  const [dupGroups, setDupGroups] = useState<DupGroup[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [similar, setSimilar] = useState<{ files: FileRow[] }[]>([]);
  const [dupJob, setDupJob] = useState<JobInfo | null>(null);

  const loadLarge = () => {
    largeFiles(minSize, 100)
      .then(setLarge)
      .catch((e) => notice(String(e)));
  };
  useEffect(loadLarge, [minSize]);

  const pollDup = () => {
    jobsList(10).then((jobs) => {
      const j = jobs.find((x) => x.kind === "duplicates");
      setDupJob(j ?? null);
      if (j?.resultJson) {
        try {
          const parsed = JSON.parse(j.resultJson);
          if (parsed.groups) setDupGroups(parsed.groups);
        } catch {
          /* 忽略解析失败 */
        }
      }
    });
  };
  useEffect(() => {
    pollDup();
    const t = window.setInterval(() => {
      if (dupJob?.state === "running" || dupJob?.state === "waiting") pollDup();
    }, 2000);
    return () => window.clearInterval(t);
  }, [dupJob?.state]);

  const startDup = () => {
    duplicatesStart(minSize, "oldest")
      .then(() => {
        notice("重复文件扫描已开始");
        pollDup();
      })
      .catch((e) => notice(String(e)));
  };

  const recycleSelected = async () => {
    const paths = [...selected];
    if (paths.length === 0) return;
    if (!window.confirm(`将把选中的 ${paths.length} 个文件移至回收站，预计释放空间。确认？`)) return;
    try {
      const token = await issueDeleteToken(paths);
      const results = await actionRecycle(paths, token);
      const failed = results.filter((r) => r.status !== "ok").length;
      notice(failed === 0 ? "已全部移至回收站" : `完成，失败 ${failed} 项`);
      setSelected(new Set());
      pollDup();
    } catch (e) {
      notice(String(e));
    }
  };

  const toggleSel = (p: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(p)) next.delete(p);
      else next.add(p);
      return next;
    });
  };

  const wastedTotal = dupGroups.reduce((acc, g) => acc + g.wasted, 0);

  return (
    <div className="page">
      <h2>空间分析</h2>
      <div className="tab-row">
        <button className={tab === "large" ? "tab-active" : ""} onClick={() => setTab("large")}>
          大文件
        </button>
        <button className={tab === "dup" ? "tab-active" : ""} onClick={() => setTab("dup")}>
          重复文件
        </button>
        <button className={tab === "similar" ? "tab-active" : ""} onClick={() => setTab("similar")}>
          相似图片（实验）
        </button>
      </div>

      {tab === "large" ? (
        <section className="card">
          <div className="card-head">
            <h3>大文件（≥ {formatBytes(minSize)}）</h3>
            <select value={minSize} onChange={(e) => setMinSize(Number(e.target.value))}>
              <option value={10 * 1024 * 1024}>10 MB</option>
              <option value={100 * 1024 * 1024}>100 MB</option>
              <option value={1024 * 1024 * 1024}>1 GB</option>
            </select>
          </div>
          <table className="data-table">
            <thead>
              <tr>
                <th>名称</th>
                <th>路径</th>
                <th>修改时间</th>
                <th className="col-right">大小</th>
              </tr>
            </thead>
            <tbody>
              {large.map((f) => (
                <tr key={f.fileId}>
                  <td>{f.name}</td>
                  <td className="muted cell-path">{f.path}</td>
                  <td className="muted">{formatTime(f.modifiedMs)}</td>
                  <td className="col-right">{formatBytes(f.size)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      ) : null}

      {tab === "dup" ? (
        <section className="card">
          <div className="card-head">
            <h3>重复文件（大小分组 → 快速指纹 → 完整 BLAKE3）</h3>
            <button className="btn btn-primary" onClick={startDup}>
              开始扫描
            </button>
          </div>
          {dupJob && (dupJob.state === "running" || dupJob.state === "waiting") ? (
            <div className="progress-line">
              <div className="progress-bar">
                <div style={{ width: `${Math.round(dupJob.progress * 100)}%` }} />
              </div>
              <span>{dupJob.message ?? "扫描中"}</span>
            </div>
          ) : null}
          {dupGroups.length > 0 ? (
            <p className="summary-line">
              发现 {dupGroups.length} 组重复 · 可释放约 {formatBytes(wastedTotal)} · 已选 {selected.size} 个
            </p>
          ) : null}
          {dupGroups.map((g, gi) => (
            <div key={g.hash} className="dup-group">
              <div className="dup-head">
                组 {gi + 1} · {g.files.length} 个文件 · 每个 {formatBytes(g.files[0]?.size ?? 0)}
              </div>
              {g.files.map((f) => (
                <label key={f.fileId} className="dup-file">
                  <input
                    type="checkbox"
                    checked={selected.has(f.path)}
                    onChange={() => toggleSel(f.path)}
                  />
                  <span className="dup-name">{f.name}</span>
                  <span className="muted cell-path">{f.path}</span>
                  <span className="muted">{formatTime(f.modifiedMs)}</span>
                </label>
              ))}
            </div>
          ))}
          {dupGroups.length > 0 ? (
            <div className="dup-footer">
              <span>已选 {selected.size} 个文件</span>
              <button className="btn btn-primary" onClick={() => void recycleSelected()}>
                移至回收站
              </button>
            </div>
          ) : null}
        </section>
      ) : null}

      {tab === "similar" ? (
        <section className="card">
          <div className="card-head">
            <h3>相似图片（dHash 感知哈希 · 独立实验，不与字节重复混淆）</h3>
            <button
              className="btn btn-primary"
              onClick={() =>
                similarImages("D:\\", 8)
                  .then(setSimilar)
                  .catch((e) => notice(String(e)))
              }
            >
              扫描 D 盘
            </button>
          </div>
          {similar.map((g, i) => (
            <div key={i} className="dup-group">
              <div className="dup-head">组 {i + 1} · {g.files.length} 张相似图片</div>
              {g.files.map((f) => (
                <div key={f.fileId} className="dup-file">
                  <span className="dup-name">{f.name}</span>
                  <span className="muted cell-path">{f.path}</span>
                </div>
              ))}
            </div>
          ))}
        </section>
      ) : null}
    </div>
  );
}
