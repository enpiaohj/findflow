import { actionCopyPath, actionLocate, actionOpen, formatBytes, formatTime, type FileRow } from "../api";
import FileIcon from "../components/FileIcon";
import { CopyIcon, LocateIcon, OpenIcon } from "../components/Icons";
import { PreviewBody, typeLabel, usePreviewPayload } from "./PreviewOverlay";

interface Props {
  item: FileRow | null;
  onNotice: (message: string) => void;
  /** 窗格宽度（px），由搜索窗拖拽分隔条决定 */
  width: number;
}

/** 预览窗格：搜索窗右侧常驻，随选中项变化（默认关闭，由筛选栏按钮或 Alt+P 开启）。 */
export default function PreviewPane({ item, onNotice, width }: Props) {
  if (!item) {
    return (
      <aside className="pane" style={{ flexBasis: width }}>
        <div className="pane-empty">选择文件后在此预览</div>
      </aside>
    );
  }
  return <PaneContent key={item.fileId} item={item} onNotice={onNotice} width={width} />;
}

function PaneContent({ item, onNotice, width }: { item: FileRow; onNotice: (m: string) => void; width: number }) {
  // 延迟 160ms：按住方向键快速滚动时不为每一项都生成预览
  const { payload, error } = usePreviewPayload(item.path, 160);
  const run = (p: Promise<string>) => p.then((m) => onNotice(m)).catch((e) => onNotice(String(e)));
  return (
    <aside className="pane" style={{ flexBasis: width }}>
      <div className="pane-body">
        <PreviewBody item={item} payload={payload} error={error} />
      </div>
      <div className="pane-info">
        <div className="pane-head">
          <FileIcon path={item.path} ext={item.ext} isDir={item.isDir} kind={item.kind} size={28} />
          <div className="pane-head-text">
            <div className="pane-title" title={item.name}>
              {item.name}
            </div>
            <div className="pane-type">{typeLabel(item)}</div>
          </div>
        </div>
        <dl className="pane-props">
          {!item.isDir ? (
            <>
              <dt>大小</dt>
              <dd>{formatBytes(payload?.size ?? item.size)}</dd>
            </>
          ) : null}
          <dt>修改</dt>
          <dd>{formatTime(payload?.modifiedMs ?? item.modifiedMs)}</dd>
          <dt>路径</dt>
          <dd className="pane-path" title={item.path}>
            {item.path}
          </dd>
        </dl>
        <div className="pane-actions">
          <button onClick={() => run(actionOpen(item.path))}>
            <OpenIcon size={16} />
            打开
          </button>
          <button onClick={() => run(actionLocate(item.path))}>
            <LocateIcon size={16} />
            定位
          </button>
          <button onClick={() => run(actionCopyPath(item.path, "full"))}>
            <CopyIcon size={16} />
            复制路径
          </button>
        </div>
      </div>
    </aside>
  );
}
