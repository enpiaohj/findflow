const BADGES: Record<string, { label: string; className: string }> = {
  doc: { label: "W", className: "badge-doc" },
  sheet: { label: "X", className: "badge-sheet" },
  pdf: { label: "P", className: "badge-pdf" },
  folder: { label: "F", className: "badge-folder" },
  image: { label: "I", className: "badge-image" },
  video: { label: "V", className: "badge-video" },
  audio: { label: "A", className: "badge-image" },
  archive: { label: "Z", className: "badge-video" },
  other: { label: "·", className: "badge-folder" },
};

interface Props {
  kind: string;
  large?: boolean;
}

export default function KindBadge({ kind, large = false }: Props) {
  const badge = BADGES[kind] ?? BADGES.other;
  return (
    <span className={"kind-badge " + badge.className + (large ? " kind-badge-large" : "")}>
      {badge.label}
    </span>
  );
}
