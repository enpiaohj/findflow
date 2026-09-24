import { getCurrentWindow } from "@tauri-apps/api/window";
import QuickApp from "./quick/QuickApp";
import ManageApp from "./manage/ManageApp";

// 双窗口路由：以 Tauri 窗口 label 区分（manage = 完整管理窗），不依赖 URL 参数。
const isManage = getCurrentWindow().label === "manage";

export default function App() {
  return isManage ? <ManageApp /> : <QuickApp />;
}
