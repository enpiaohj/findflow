import { useEffect, useState } from "react";
import { previewGet, actionOpen, actionLocate, actionCopyPath, formatBytes, formatTime, type PreviewPayload } from "../api";

interface Props {
  path: string;
  onClose: () => void;
  onNotice: (message: string) => void;
}

export default function PreviewOverlay({ path, onClose, onNotice }: Props) {
  const [payload, setPayload] = useState<PreviewPayload | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setPayload(null);
    setError(null);
    previewGet(path)
      .then(setPayload)
      .catch((e) => setError(String(e)));
  }, [path]);

  return (
    <div className="preview-overlay">
      <div className="preview-panel">
        <div className="preview-toolbar">
          <span className="preview-title">{payload?.name ?? path.split("\\").pop()}</span>
          <div className="preview-actions">
            <button
              onClick={() =>
                actionOpen(path)
                  .then((m) => onNotice(m))
                  .catch((e) => onNotice(String(e)))
              }
            >
              打开
            </button>
            <button
              onClick={() =>
                actionLocate(path)
                  .then((m) => onNotice(m))
                  .catch((e) => onNotice(String(e)))
              }
            >
              定位
            </button>
            <button
              onClick={() =>
                actionCopyPath(path, "full")
                  .then((m) => onNotice(m))
                  .catch((e) => onNotice(String(e)))
              }
            >
              复制路径
            </button>
            <button className="preview-close" onClick={onClose}>
              ✕
            </button>
          </div>
        </div>
        <div className="preview-body">
          {error ? <div className="preview-fallback">预览失败：{error}</div> : null}
          {!error && !payload ? <div className="preview-fallback">生成预览中…</div> : null}
          {payload?.kind === "image" && payload.data ? (
            <img src={`data:image/png;base64,${payload.data}`} alt={payload.name} />
          ) : null}
          {payload?.kind === "thumbnail" && payload.data ? (
            <img src={`data:image/png;base64,${payload.data}`} alt={payload.name} />
          ) : null}
          {payload?.kind === "text" && payload.data !== null ? (
            <pre className="preview-text">{payload.data}</pre>
          ) : null}
          {payload && payload.kind === "none" ? (
            <div className="preview-fallback">
              <p>该类型暂不支持预览</p>
              <p className="preview-hint">{payload.reason ?? "点击上方“打开”使用默认应用查看"}</p>
            </div>
          ) : null}
          {payload && payload.kind !== "text" && payload.reason ? (
            <div className="preview-reason">{payload.reason}</div>
          ) : null}
        </div>
        <div className="preview-meta">
          <span>{formatBytes(payload?.size ?? 0)}</span>
          <span>{formatTime(payload?.modifiedMs ?? 0)}</span>
          <span className="preview-path">{path}</span>
        </div>
      </div>
    </div>
  );
}
