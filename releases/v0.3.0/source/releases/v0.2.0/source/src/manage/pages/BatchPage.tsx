import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import type { PageCtx } from "../ManageBody";
import { useConfirm } from "../../components/ConfirmDialog";
import {
  batchRenamePreview,
  batchRenameExecute,
  batchRenameUndo,
  batchMovePreview,
  batchMoveExecute,
  organizeSuggest,
  organizeExecute,
  type PlanItem,
} from "../../api";

export default function BatchPage({ notice }: PageCtx) {
  const { confirm, confirmEl } = useConfirm();
  const [tab, setTab] = useState<"rename" | "move" | "organize">("rename");
  const [pathsText, setPathsText] = useState("");
  const [items, setItems] = useState<PlanItem[]>([]);
  const [lastOp, setLastOp] = useState<number | null>(null);
  const [targetDir, setTargetDir] = useState("");
  const [conflict, setConflict] = useState("keepBoth");
  const [rule, setRule] = useState({
    find: "",
    replace: "",
    prefix: "",
    suffix: "",
    dateFormat: "",
    numbering: false,
    numberingDigits: 3,
    regex: false,
  });
  const [orgSource, setOrgSource] = useState("");
  const [orgType, setOrgType] = useState("extension");
  const [orgTarget, setOrgTarget] = useState("");

  const parsePaths = () =>
    pathsText
      .split(/\r?\n/)
      .map((s) => s.trim().replace(/^"|"$/g, ""))
      .filter(Boolean);

  const pickFiles = async () => {
    const picked = await open({ multiple: true, directory: false });
    if (Array.isArray(picked)) {
      setPathsText((prev) => (prev ? prev + "\n" : "") + picked.join("\n"));
    }
  };

  const paths = parsePaths();

  const previewRename = () => {
    batchRenamePreview(paths, rule)
      .then(setItems)
      .catch((e) => notice(String(e)));
  };

  const executeRename = async () => {
    const okItems = items.filter((i) => i.status === "ok");
    if (okItems.length === 0) return;
    if (!(await confirm(`确认重命名 ${okItems.length} 项？`, "问题项将跳过；完成后可一次性撤销。"))) return;
    batchRenameExecute(items)
      .then((r) => {
        setLastOp(r.operationId);
        notice(`完成：成功 ${r.ok}，失败 ${r.failed}，跳过 ${r.skipped}`);
        setItems([]);
      })
      .catch((e) => notice(String(e)));
  };

  const previewMove = () => {
    if (!targetDir) {
      notice("请先选择目标目录");
      return;
    }
    batchMovePreview(paths, targetDir)
      .then(setItems)
      .catch((e) => notice(String(e)));
  };

  const executeMove = async () => {
    const okItems = items.filter((i) => i.status === "ok");
    if (okItems.length === 0) return;
    if (!(await confirm(`确认移动 ${okItems.length} 项到 ${targetDir}？`, "跨卷移动将先复制、校验后删除源文件。"))) return;
    batchMoveExecute(items, conflict)
      .then((r) => {
        notice(`完成：成功 ${r.ok}，失败 ${r.failed}，跳过 ${r.skipped}`);
        setItems([]);
      })
      .catch((e) => notice(String(e)));
  };

  const statusBadge = (s: string) =>
    s === "ok" ? "badge-ok" : s === "conflict" ? "badge-warn" : s === "illegal" ? "badge-err" : "badge-muted";

  return (
    <div className="page">
      <h2>批量处理</h2>
      <div className="tab-row">
        <button className={tab === "rename" ? "tab-active" : ""} onClick={() => setTab("rename")}>
          批量重命名
        </button>
        <button className={tab === "move" ? "tab-active" : ""} onClick={() => setTab("move")}>
          批量移动
        </button>
        <button className={tab === "organize" ? "tab-active" : ""} onClick={() => setTab("organize")}>
          智能整理
        </button>
      </div>

      <section className="card">
        <div className="card-head">
          <h3>文件列表（每行一个路径）</h3>
          <button onClick={() => void pickFiles()}>选择文件…</button>
        </div>
        <textarea
          className="paths-input"
          rows={5}
          value={pathsText}
          placeholder={"D:\\项目资料\\AD升级\\方案.docx\nD:\\IT运维\\AD\\清单.xlsx"}
          onChange={(e) => setPathsText(e.target.value)}
        />
        <p className="muted">已输入 {paths.length} 个路径 · 执行前必须预演确认</p>
      </section>

      {tab === "rename" ? (
        <section className="card">
          <div className="card-head">
            <h3>重命名规则</h3>
            <button onClick={previewRename}>实时预演</button>
          </div>
          <div className="rule-grid">
            <label>
              查找
              <input value={rule.find} onChange={(e) => setRule({ ...rule, find: e.target.value })} />
            </label>
            <label>
              替换为
              <input value={rule.replace} onChange={(e) => setRule({ ...rule, replace: e.target.value })} />
            </label>
            <label>
              添加前缀
              <input value={rule.prefix} onChange={(e) => setRule({ ...rule, prefix: e.target.value })} />
            </label>
            <label>
              添加后缀
              <input value={rule.suffix} onChange={(e) => setRule({ ...rule, suffix: e.target.value })} />
            </label>
            <label>
              日期格式（如 yyyy-MM-dd）
              <input value={rule.dateFormat} onChange={(e) => setRule({ ...rule, dateFormat: e.target.value })} />
            </label>
            <label className="rule-check">
              <input
                type="checkbox"
                checked={rule.numbering}
                onChange={(e) => setRule({ ...rule, numbering: e.target.checked })}
              />
              自动编号（{rule.numberingDigits} 位）
            </label>
          </div>
          {items.length > 0 ? (
            <>
              <table className="data-table">
                <thead>
                  <tr>
                    <th>状态</th>
                    <th>原名称</th>
                    <th />
                    <th>新名称</th>
                  </tr>
                </thead>
                <tbody>
                  {items.map((it, i) => (
                    <tr key={i}>
                      <td>
                        <span className={"status-badge " + statusBadge(it.status)}>
                          {it.status === "ok"
                            ? "可执行"
                            : it.status === "conflict"
                              ? "名称冲突"
                              : it.status === "illegal"
                                ? "非法字符"
                                : it.status === "unchanged"
                                  ? "无变化"
                                  : it.status}
                        </span>
                      </td>
                      <td>{it.oldPath.split("\\").pop()}</td>
                      <td className="muted">→</td>
                      <td>{it.newPath.split("\\").pop()}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <div className="dup-footer">
                {lastOp ? (
                  <button onClick={() => batchRenameUndo(lastOp).then((r) => notice(`已撤销 ${r.undone} 项，冲突 ${r.conflicts} 项`))}>
                    撤销上一次批量重命名（#{lastOp}）
                  </button>
                ) : null}
                <button className="btn btn-primary" onClick={executeRename}>
                  确认重命名 {items.filter((i) => i.status === "ok").length} 项
                </button>
              </div>
            </>
          ) : null}
        </section>
      ) : null}

      {tab === "move" ? (
        <section className="card">
          <div className="card-head">
            <h3>移动到目标目录</h3>
            <button
              onClick={async () => {
                const d = await open({ directory: true, multiple: false });
                if (typeof d === "string") setTargetDir(d);
              }}
            >
              选择目录…
            </button>
          </div>
          <p className="muted">目标：{targetDir || "未选择"}</p>
          <label className="muted">
            冲突处理
            <select value={conflict} onChange={(e) => setConflict(e.target.value)}>
              <option value="skip">跳过</option>
              <option value="overwrite">覆盖</option>
              <option value="keepBoth">保留两者</option>
              <option value="onlyNewer">仅保留较新</option>
            </select>
          </label>
          <div className="trial-actions">
            <button onClick={previewMove}>预演</button>
            <button className="btn btn-primary" onClick={executeMove}>
              确认移动 {items.filter((i) => i.status === "ok").length} 项
            </button>
          </div>
          {items.length > 0 ? (
            <table className="data-table">
              <thead>
                <tr>
                  <th>状态</th>
                  <th>源</th>
                  <th>目标</th>
                </tr>
              </thead>
              <tbody>
                {items.map((it, i) => (
                  <tr key={i}>
                    <td>
                      <span className={"status-badge " + statusBadge(it.status)}>{it.status}</span>
                    </td>
                    <td className="cell-path">{it.oldPath}</td>
                    <td className="cell-path">{it.newPath}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : null}
        </section>
      ) : null}

      {confirmEl}
      {tab === "organize" ? (
        <section className="card">
          <div className="card-head">
            <h3>规则式整理（默认仅建议）</h3>
          </div>
          <div className="rule-grid">
            <label>
              来源目录
              <input value={orgSource} onChange={(e) => setOrgSource(e.target.value)} placeholder="D:\下载" />
            </label>
            <label>
              规则
              <select value={orgType} onChange={(e) => setOrgType(e.target.value)}>
                <option value="extension">按扩展名分目录</option>
                <option value="date_ym">按修改年月分目录</option>
              </select>
            </label>
            <label>
              目标根目录
              <input value={orgTarget} onChange={(e) => setOrgTarget(e.target.value)} placeholder="D:\整理" />
            </label>
          </div>
          <div className="trial-actions">
            <button
              onClick={() =>
                organizeSuggest(orgSource, orgType, orgTarget)
                  .then(setItems)
                  .catch((e) => notice(String(e)))
              }
            >
              生成建议（最近 100 个文件）
            </button>
            <button
              className="btn btn-primary"
              onClick={async () => {
                const okItems = items.filter((i) => i.status === "ok");
                if (okItems.length === 0) {
                  notice("没有可执行的整理项");
                  return;
                }
                if (!(await confirm(`确认按建议移动 ${okItems.length} 项？`))) return;
                organizeExecute(items)
                  .then((r) => notice(`整理完成：成功 ${r.ok}，失败 ${r.failed}`))
                  .catch((e) => notice(String(e)));
              }}
            >
              执行整理
            </button>
          </div>
          {items.map((it, i) => (
            <div key={i} className="dup-file">
              <span className="dup-name">{it.oldPath}</span>
              <span className="muted">→ {it.newPath}</span>
            </div>
          ))}
        </section>
      ) : null}
    </div>
  );
}
