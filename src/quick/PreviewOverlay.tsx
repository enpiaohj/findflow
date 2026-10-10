import { useEffect, useState } from "react";
import {
  previewGet,
  actionOpen,
  actionLocate,
  actionCopyPath,
  formatBytes,
  formatTime,
  type FileRow,
  type PreviewPayload,
} from "../api";
import FileIcon from "../components/FileIcon";
import { CloseIcon, CopyIcon, LocateIcon, OpenIcon } from "../components/Icons";

interface Props {
  item: FileRow;
  onClose: () => void;
  onNotice: (message: string) => void;
}

const KIND_LABELS: Record<string, string> = {
  doc: "文档",
  sheet: "表格",
  pdf: "PDF 文档",
  image: "图片",
  video: "视频",
  audio: "音频",
  archive: "压缩包",
  folder: "文件夹",
};

export function typeLabel(item: FileRow): string {
  if (item.isDir) return "文件夹";
  if (item.kind === "pdf") return KIND_LABELS.pdf;
  const ext = item.ext ? item.ext.toUpperCase() + " " : "";
  return ext + (KIND_LABELS[item.kind] ?? "文件");
}

/** 取预览内容。`delay` 用于预览窗格：连续切换选中项时只为停下来的那一项生成预览。 */
export function usePreviewPayload(path: string, delay = 0) {
  const [payload, setPayload] = useState<PreviewPayload | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setPayload(null);
    setError(null);
    let alive = true;
    const timer = window.setTimeout(() => {
      previewGet(path)
        .then((p) => alive && setPayload(p))
        .catch((e) => alive && setError(String(e)));
    }, delay);
    return () => {
      alive = false;
      window.clearTimeout(timer);
    };
  }, [path, delay]);

  return { payload, error };
}

/** 预览内容区（图片 / 缩略图 / 文本 / 暂不支持），浮层与预览窗格共用。 */
export function PreviewBody({
  item,
  payload,
  error,
}: {
  item: FileRow;
  payload: PreviewPayload | null;
  error: string | null;
}) {
  return (
    <>
      {error ? <div className="preview-fallback">预览失败：{error}</div> : null}
      {!error && !payload ? <div className="preview-fallback">正在生成预览…</div> : null}
      {(payload?.kind === "image" || payload?.kind === "thumbnail") && payload.data ? (
        <img src={`data:image/png;base64,${payload.data}`} alt={payload.name} />
      ) : null}
      {payload?.kind === "text" && payload.data !== null ? <pre className="preview-text">{payload.data}</pre> : null}
      {payload && payload.kind === "none" ? (
        <div className="preview-fallback">
          <FileIcon path={item.path} ext={item.ext} isDir={item.isDir} kind={item.kind} size={96} />
          <p>该类型暂不支持内容预览</p>
          <p className="preview-hint">{payload.reason ?? "按 Enter 使用默认应用打开"}</p>
        </div>
      ) : null}
      {payload && payload.kind !== "text" && payload.kind !== "none" && payload.reason ? (
        <div className="preview-reason">{payload.reason}</div>
      ) : null}
    </>
  );
}

export default function PreviewOverlay({ item, onClose, onNotice }: Props) {
  const path = item.path;
  const { payload, error } = usePreviewPayload(path);

  const run = (p: Promise<string>) => p.then((m) => onNotice(m)).catch((e) => onNotice(String(e)));

  return (
    <div className="preview-overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="preview-panel">
        <div className="preview-main">
          <div className="preview-body">
            <PreviewBody item={item} payload={payload} error={error} />
          </div>
          <div className="preview-footer">
            <kbd>←</kbd>
            <kbd>→</kbd> 切换文件
            <i />
            <kbd>Enter</kbd> 打开
            <i />
            <kbd>Space</kbd> 或 <kbd>Esc</kbd> 关闭
          </div>
        </div>
        <aside className="preview-side">
          <div className="preview-icon">
            <FileIcon path={path} ext={item.ext} isDir={item.isDir} kind={item.kind} size={48} />
          </div>
          <div className="preview-title">{item.name}</div>
          <div className="preview-type">{typeLabel(item)}</div>
          <dl className="preview-props">
            {!item.isDir ? (
              <>
                <dt>大小</dt>
                <dd>{formatBytes(payload?.size ?? item.size)}</dd>
              </>
            ) : null}
            <dt>修改时间</dt>
            <dd>{formatTime(payload?.modifiedMs ?? item.modifiedMs)}</dd>
            <dt>路径</dt>
            <dd>{path}</dd>
          </dl>
          <div className="preview-actions">
            <button onClick={() => run(actionOpen(path))}>
              <OpenIcon size={18} />
              打开
              <kbd>Enter</kbd>
            </button>
            <button onClick={() => run(actionLocate(path))}>
              <LocateIcon size={18} />
              定位
            </button>
            <button onClick={() => run(actionCopyPath(path, "full"))}>
              <CopyIcon size={18} />
              复制路径
            </button>
          </div>
        </aside>
        <button className="preview-close" title="关闭（Esc）" onClick={onClose}>
          <CloseIcon size={16} />
        </button>
      </div>
    </div>
  );
}
