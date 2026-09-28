import type { ReactNode } from "react";

/** 统一线性图标：20 网格、1.5 线宽、圆角端点（Fluent 风格），颜色继承 currentColor。 */
function Svg({ size = 16, children, fill = false }: { size?: number; children: ReactNode; fill?: boolean }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 20 20"
      fill={fill ? "currentColor" : "none"}
      stroke={fill ? "none" : "currentColor"}
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {children}
    </svg>
  );
}

export function SearchIcon({ size = 18 }: { size?: number }) {
  return (
    <Svg size={size}>
      <circle cx="8.5" cy="8.5" r="5.25" />
      <path d="M12.5 12.5 17 17" />
    </Svg>
  );
}

export function GearIcon({ size = 16 }: { size?: number }) {
  return (
    <Svg size={size}>
      <path d="M8.6 2.6h2.8l.4 2.1 1.5.9 2-.8 1.4 2.4-1.6 1.4v1.8l1.6 1.4-1.4 2.4-2-.8-1.5.9-.4 2.1H8.6l-.4-2.1-1.5-.9-2 .8-1.4-2.4 1.6-1.4V9.2L3.3 7.8l1.4-2.4 2 .8 1.5-.9z" />
      <circle cx="10" cy="10" r="2.4" />
    </Svg>
  );
}

export function MinimizeIcon() {
  return (
    <Svg size={14}>
      <path d="M4.5 10h11" />
    </Svg>
  );
}

export function MaximizeIcon() {
  return (
    <Svg size={14}>
      <rect x="4.5" y="4.5" width="11" height="11" rx="1.8" />
    </Svg>
  );
}

export function CloseIcon({ size = 14 }: { size?: number }) {
  return (
    <Svg size={size}>
      <path d="M5 5l10 10M15 5 5 15" />
    </Svg>
  );
}

export function OpenIcon({ size = 16 }: { size?: number }) {
  return (
    <Svg size={size}>
      <path d="M9 4.5H5.5a1.5 1.5 0 0 0-1.5 1.5v8.5A1.5 1.5 0 0 0 5.5 16H14a1.5 1.5 0 0 0 1.5-1.5V11" />
      <path d="M12 3.5h4.5V8M16.5 3.5 9.5 10.5" />
    </Svg>
  );
}

export function LocateIcon({ size = 16 }: { size?: number }) {
  return (
    <Svg size={size}>
      <path d="M2.8 6.2c0-1 .8-1.7 1.7-1.7h3.3l1.8 2h5.9c1 0 1.7.8 1.7 1.7v6.1c0 1-.8 1.7-1.7 1.7h-11c-1 0-1.7-.8-1.7-1.7z" />
    </Svg>
  );
}

export function MoreIcon({ size = 16 }: { size?: number }) {
  return (
    <Svg size={size} fill>
      <circle cx="4.5" cy="10" r="1.4" />
      <circle cx="10" cy="10" r="1.4" />
      <circle cx="15.5" cy="10" r="1.4" />
    </Svg>
  );
}

export function CopyIcon({ size = 16 }: { size?: number }) {
  return (
    <Svg size={size}>
      <rect x="7" y="7" width="9.5" height="9.5" rx="1.8" />
      <path d="M13 7V5.3c0-1-.8-1.8-1.8-1.8H5.3c-1 0-1.8.8-1.8 1.8v5.9c0 1 .8 1.8 1.8 1.8H7" />
    </Svg>
  );
}

/* ---------- 管理窗导航 ---------- */

export function ChartIcon({ size = 18 }: { size?: number }) {
  return (
    <Svg size={size}>
      <path d="M10 3a7 7 0 1 0 7 7h-7z" />
      <path d="M12.5 2.8a5.6 5.6 0 0 1 4.7 4.7h-4.7z" />
    </Svg>
  );
}

export function LayersIcon({ size = 18 }: { size?: number }) {
  return (
    <Svg size={size}>
      <path d="M10 3 17 6.8 10 10.6 3 6.8z" />
      <path d="M3 10.2 10 14l7-3.8M3 13.6l7 3.8 7-3.8" />
    </Svg>
  );
}

export function PulseIcon({ size = 18 }: { size?: number }) {
  return (
    <Svg size={size}>
      <path d="M2.5 10h3.2l2-5 3.4 10 2-5h4.4" />
    </Svg>
  );
}

export function StarIcon({ size = 18 }: { size?: number }) {
  return (
    <Svg size={size}>
      <path d="m10 3 2.1 4.4 4.8.6-3.5 3.3.9 4.8L10 13.8l-4.3 2.3.9-4.8L3.1 8l4.8-.6z" />
    </Svg>
  );
}

export function MonitorIcon({ size = 18 }: { size?: number }) {
  return (
    <Svg size={size}>
      <rect x="2.8" y="3.8" width="14.4" height="9.6" rx="1.6" />
      <path d="M7 16.5h6M10 13.4v3.1" />
    </Svg>
  );
}

export function ShieldIcon({ size = 18 }: { size?: number }) {
  return (
    <Svg size={size}>
      <path d="M10 2.8 16 5v4.6c0 3.6-2.6 6.3-6 7.6-3.4-1.3-6-4-6-7.6V5z" />
      <path d="m7.4 10 1.8 1.8 3.5-3.6" />
    </Svg>
  );
}

export function RefreshIcon({ size = 18 }: { size?: number }) {
  return (
    <Svg size={size}>
      <path d="M16 10a6 6 0 1 1-1.8-4.3" />
      <path d="M16.2 3.5v3.2H13" />
    </Svg>
  );
}

export function ArrowLeftIcon({ size = 16 }: { size?: number }) {
  return (
    <Svg size={size}>
      <path d="M12.5 4.5 7 10l5.5 5.5" />
    </Svg>
  );
}

export function InfoIcon({ size = 18 }: { size?: number }) {
  return (
    <Svg size={size}>
      <circle cx="10" cy="10" r="6.5" />
      <path d="M10 9v4.5" />
      <path d="M10 6.6v.2" />
    </Svg>
  );
}
