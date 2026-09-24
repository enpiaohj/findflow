import type { FileKind } from "../types";

const BADGES: Record<FileKind, { label: string; className: string }> = {
  doc: { label: "W", className: "badge-doc" },
  sheet: { label: "X", className: "badge-sheet" },
  pdf: { label: "P", className: "badge-pdf" },
  folder: { label: "F", className: "badge-folder" },
  image: { label: "I", className: "badge-image" },
  video: { label: "V", className: "badge-video" },
};

interface Props {
  kind: FileKind;
  large?: boolean;
}

export default function KindBadge({ kind, large = false }: Props) {
  const badge = BADGES[kind];
  return (
    <span className={"kind-badge " + badge.className + (large ? " kind-badge-large" : "")}>
      {badge.label}
    </span>
  );
}
