import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import TitleBar from "../components/TitleBar";
import { Toast } from "../components/Toast";
import IndexPage from "./pages/IndexPage";
import SpacePage from "./pages/SpacePage";
import BatchPage from "./pages/BatchPage";
import TasksPage from "./pages/TasksPage";
import LibraryPage from "./pages/LibraryPage";
import { AppearancePage, PrivacyPage, UpdatePage } from "./pages/SettingsPages";

const NAV = [
  { id: "index", label: "索引与搜索", el: IndexPage },
  { id: "space", label: "空间分析", el: SpacePage },
  { id: "batch", label: "批量处理", el: BatchPage },
  { id: "tasks", label: "任务与诊断", el: TasksPage },
  { id: "library", label: "收藏与标签", el: LibraryPage },
  { id: "appearance", label: "外观与快捷键", el: AppearancePage },
  { id: "privacy", label: "隐私与安全", el: PrivacyPage },
  { id: "update", label: "更新", el: UpdatePage },
] as const;

type PageCtx = { notice: (m: string) => void; refresh: () => void };
export type { PageCtx };

export default function ManageApp() {
  const [page, setPage] = useState<(typeof NAV)[number]["id"]>("index");
  const [toast, setToast] = useState<string | null>(null);
  const [tick, setTick] = useState(0);
  const appWindow = getCurrentWindow();

  const notice = (m: string) => {
    setToast(m);
    window.setTimeout(() => setToast(null), 2600);
  };
  const refresh = () => setTick((t) => t + 1);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") void appWindow.hide();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [appWindow]);

  const Active = NAV.find((n) => n.id === page)!.el;

  return (
    <div className="app manage-app">
      <TitleBar />
      <div className="manage-body">
        <nav className="manage-nav">
          {NAV.map((n) => (
            <button
              key={n.id}
              className={"nav-item" + (page === n.id ? " nav-active" : "")}
              onClick={() => setPage(n.id)}
            >
              {n.label}
            </button>
          ))}
        </nav>
        <main className="manage-content" key={tick}>
          <Active notice={notice} refresh={refresh} />
        </main>
      </div>
      {toast ? <Toast message={toast} /> : null}
    </div>
  );
}
