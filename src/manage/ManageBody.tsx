import { useState } from "react";
import IndexPage from "./pages/IndexPage";
import SpacePage from "./pages/SpacePage";
import BatchPage from "./pages/BatchPage";
import TasksPage from "./pages/TasksPage";
import LibraryPage from "./pages/LibraryPage";
import { AppearancePage, PrivacyPage, AboutPage } from "./pages/SettingsPages";
import {
  ChartIcon,
  InfoIcon,
  LayersIcon,
  MonitorIcon,
  PulseIcon,
  SearchIcon,
  ShieldIcon,
  StarIcon,
} from "../components/Icons";

const NAV = [
  { id: "index", label: "索引与搜索", el: IndexPage, icon: SearchIcon },
  { id: "space", label: "空间分析", el: SpacePage, icon: ChartIcon },
  { id: "batch", label: "批量处理", el: BatchPage, icon: LayersIcon },
  { id: "tasks", label: "任务与诊断", el: TasksPage, icon: PulseIcon },
  { id: "library", label: "收藏与标签", el: LibraryPage, icon: StarIcon },
  { id: "appearance", label: "外观与快捷键", el: AppearancePage, icon: MonitorIcon },
  { id: "privacy", label: "隐私与安全", el: PrivacyPage, icon: ShieldIcon },
  { id: "about", label: "关于", el: AboutPage, icon: InfoIcon },
] as const;

export type PageCtx = { notice: (m: string) => void; refresh: () => void };

export default function ManageBody({ notice, initialPage }: { notice: (m: string) => void; initialPage?: string }) {
  const [page, setPage] = useState<(typeof NAV)[number]["id"]>(
    (initialPage as (typeof NAV)[number]["id"]) ?? "index"
  );
  const [tick, setTick] = useState(0);
  const refresh = () => setTick((t) => t + 1);
  const Active = NAV.find((n) => n.id === page)!.el;

  return (
    <div className="manage-body">
      <nav className="manage-nav">
        {NAV.map((n) => (
          <button
            key={n.id}
            className={"nav-item" + (page === n.id ? " nav-active" : "")}
            onClick={() => setPage(n.id)}
          >
            <n.icon size={18} />
            {n.label}
          </button>
        ))}
      </nav>
      <main className="manage-content" key={tick}>
        <Active notice={notice} refresh={refresh} />
      </main>
    </div>
  );
}
