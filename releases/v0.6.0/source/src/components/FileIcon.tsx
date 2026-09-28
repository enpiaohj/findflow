import { useState } from "react";
import { useFileIcon, useThumbnail, type IconSubject } from "../fileIcons";

/** 内置图标配色（系统图标取不到时使用），按文件大类区分。 */
const KIND_COLORS: Record<string, string> = {
  doc: "#2B6CD4",
  sheet: "#1F8A4C",
  pdf: "#D93025",
  image: "#0EA5E9",
  video: "#8B5CF6",
  audio: "#DB2777",
  archive: "#B7791F",
  other: "#6B7280",
};

interface FallbackProps {
  kind: string;
  ext: string;
  isDir: boolean;
  size: number;
}

/** 内置矢量图标：文件夹外形，或带扩展名标签的文档外形。 */
export function FallbackIcon({ kind, ext, isDir, size }: FallbackProps) {
  if (isDir || kind === "folder") {
    return (
      <svg width={size} height={size} viewBox="0 0 32 32" aria-hidden="true">
        <path d="M3 8.5C3 7.1 4.1 6 5.5 6h7l2.6 2.6H26.5c1.4 0 2.5 1.1 2.5 2.5v13.4c0 1.4-1.1 2.5-2.5 2.5h-21C4.1 27 3 25.9 3 24.5z" fill="#F2B84B" />
        <path d="M3 12.5c0-1.4 1.1-2.5 2.5-2.5h21c1.4 0 2.5 1.1 2.5 2.5v12c0 1.4-1.1 2.5-2.5 2.5h-21C4.1 27 3 25.9 3 24.5z" fill="#FFD27A" />
      </svg>
    );
  }
  const color = KIND_COLORS[kind] ?? KIND_COLORS.other;
  const label = ext.slice(0, 4).toUpperCase();
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" aria-hidden="true">
      <path d="M7.5 2.5h12l7 7v18a2 2 0 0 1-2 2h-17a2 2 0 0 1-2-2v-23a2 2 0 0 1 2-2z" fill="#FFFFFF" stroke="#C9CFD8" />
      <path d="M19.5 2.5v5a2 2 0 0 0 2 2h5" fill="#EEF1F5" stroke="#C9CFD8" />
      {label ? (
        <>
          <rect x="3" y="17" width="21" height="9" rx="2" fill={color} />
          <text
            x="13.5"
            y="23.6"
            textAnchor="middle"
            fontSize={label.length > 3 ? 5.6 : 6.6}
            fontWeight="700"
            fontFamily="Segoe UI, sans-serif"
            fill="#FFFFFF"
          >
            {label}
          </text>
        </>
      ) : (
        <rect x="9" y="15" width="12" height="2" rx="1" fill="#C9CFD8" />
      )}
    </svg>
  );
}

interface Props extends IconSubject {
  kind: string;
  size: number;
}

/** 文件图标：系统图标优先，取不到时显示内置图标。 */
export default function FileIcon({ path, ext, isDir, kind, size }: Props) {
  const src = useFileIcon({ path, ext, isDir }, size);
  return (
    <span className="file-icon" style={{ width: size, height: size }}>
      {src ? (
        <img src={src} width={size} height={size} alt="" draggable={false} />
      ) : (
        <FallbackIcon kind={kind} ext={ext} isDir={isDir} size={size} />
      )}
    </span>
  );
}

/** 可生成缩略图的类型：图片、视频与常见文档（由系统缩略图提供程序决定是否真的有图）。 */
const THUMB_KINDS = new Set(["image", "video", "pdf", "doc", "sheet"]);

interface ThumbProps extends IconSubject {
  kind: string;
  modifiedMs: number;
  height: number;
}

/** 大图标卡片的预览区：有系统缩略图时铺满显示，否则居中显示大号文件图标。 */
export function FileThumb({ path, ext, isDir, kind, modifiedMs, height }: ThumbProps) {
  const thumb = useThumbnail(path, modifiedMs, height * 2, !isDir && THUMB_KINDS.has(kind));
  // 原图明显小于预览区（如图标、表情）时按原尺寸居中，避免放大裁切后发虚
  const [small, setSmall] = useState(false);
  return (
    <div className={"result-thumb" + (thumb ? " has-image" : "")} style={{ height }}>
      {thumb ? (
        <img
          className={"result-thumb-img" + (small ? " small" : "")}
          src={thumb}
          alt=""
          draggable={false}
          onLoad={(e) => {
            const img = e.currentTarget;
            const dpr = window.devicePixelRatio || 1;
            setSmall(img.naturalHeight / dpr < height * 0.8 && img.naturalWidth / dpr < img.clientWidth * 0.8);
          }}
        />
      ) : (
        <FileIcon path={path} ext={ext} isDir={isDir} kind={kind} size={64} />
      )}
      {thumb && kind === "video" ? <span className="thumb-play" aria-hidden="true" /> : null}
    </div>
  );
}
