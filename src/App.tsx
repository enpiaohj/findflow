import QuickApp from "./quick/QuickApp";
import ManageApp from "./manage/ManageApp";

// 双窗口路由：管理窗以 ?window=manage 打开（FR 信息架构：两个窗口共用后台）。
const params = new URLSearchParams(window.location.search);
const isManage = params.get("window") === "manage";

export default function App() {
  return isManage ? <ManageApp /> : <QuickApp />;
}
