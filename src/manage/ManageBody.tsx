import { useState, type ComponentType } from "react";
import IndexPage from "./pages/IndexPage";
import SpacePage from "./pages/SpacePage";
import BatchPage from "./pages/BatchPage";
import TasksPage from "./pages/TasksPage";
import LibraryPage from "./pages/LibraryPage";
import RegularPage from "./pages/RegularPage";
import SearchSettingsPage from "./pages/SearchSettingsPage";
import ContentPage from "./pages/ContentPage";
import AdvancedPage from "./pages/AdvancedPage";
import { AppearanceSettingsPage, HotkeyPage, PrivacyPage, AboutPage } from "./pages/SettingsPages";
import {
  ChartIcon,
  ChevronDownIcon,
  DatabaseIcon,
  DocumentIcon,
  GearIcon,
  InfoIcon,
  KeyboardIcon,
  LayersIcon,
  MonitorIcon,
  PulseIcon,
  SearchIcon,
  ShieldIcon,
  SlidersIcon,
  StarIcon,
} from "../components/Icons";

export type PageCtx = { notice: (m: string) => void; refresh: () => void };

interface NavItem {
  id: string;
  label: string;
  el: ComponentType<PageCtx>;
  icon: ComponentType<{ size?: number }>;
}

interface NavGroup {
  id: string;
  title: string;
  items: NavItem[];
}

/** 导航分两组：设置在前，功能模块在后。 */
const NAV_GROUPS: NavGroup[] = [
  {
    id: "settings",
    title: "设置",
    items: [
      { id: "regular", label: "常规", el: RegularPage, icon: GearIcon },
      { id: "search", label: "搜索", el: SearchSettingsPage, icon: SearchIcon },
      { id: "index", label: "索引", el: IndexPage, icon: DatabaseIcon },
      { id: "content", label: "文件与内容", el: ContentPage, icon: DocumentIcon },
      { id: "appearance", label: "外观", el: AppearanceSettingsPage, icon: MonitorIcon },
      { id: "hotkey", label: "快捷键", el: HotkeyPage, icon: KeyboardIcon },
      { id: "privacy", label: "隐私与安全", el: PrivacyPage, icon: ShieldIcon },
      { id: "advanced", label: "高级", el: AdvancedPage, icon: SlidersIcon },
      { id: "about", label: "关于", el: AboutPage, icon: InfoIcon },
    ],
  },
  {
    id: "features",
    title: "功能",
    items: [
      { id: "space", label: "空间分析", el: SpacePage, icon: ChartIcon },
      { id: "batch", label: "批量处理", el: BatchPage, icon: LayersIcon },
      { id: "tasks", label: "任务", el: TasksPage, icon: PulseIcon },
      { id: "library", label: "收藏与标签", el: LibraryPage, icon: StarIcon },
    ],
  },
];

const NAV_ITEMS = NAV_GROUPS.flatMap((group) => group.items);
type NavId = string;

export default function ManageBody({ notice, initialPage }: { notice: (m: string) => void; initialPage?: string }) {
  const [page, setPage] = useState<NavId>((initialPage as NavId) ?? "index");
  const [tick, setTick] = useState(0);
  const [folded, setFolded] = useState<Record<string, boolean>>({});
  const refresh = () => setTick((t) => t + 1);
  const Active = (NAV_ITEMS.find((n) => n.id === page) ?? NAV_ITEMS[0]).el;

  return (
    <div className="manage-body">
      <nav className="manage-nav">
        {NAV_GROUPS.map((group) => (
          <div key={group.id} className="nav-group">
            <button
              className="nav-group-title"
              aria-expanded={!folded[group.id]}
              onClick={() => setFolded((f) => ({ ...f, [group.id]: !f[group.id] }))}
            >
              <ChevronDownIcon size={12} />
              {group.title}
            </button>
            {(folded[group.id] ? group.items.filter((n) => n.id === page) : group.items).map((n) => (
              <button
                key={n.id}
                className={"nav-item" + (page === n.id ? " nav-active" : "")}
                onClick={() => setPage(n.id)}
              >
                <n.icon size={18} />
                {n.label}
              </button>
            ))}
          </div>
        ))}
      </nav>
      <main className="manage-content" key={tick}>
        <Active notice={notice} refresh={refresh} />
      </main>
    </div>
  );
}
