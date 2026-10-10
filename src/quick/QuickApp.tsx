import { useCallback, useEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import TitleBar from "../components/TitleBar";
import ResultList from "../components/ResultList";
import FooterBar from "../components/FooterBar";
import { Toast } from "../components/Toast";
import { ArrowLeftIcon, ChevronDownIcon, CloseIcon, SearchIcon } from "../components/Icons";
import PreviewOverlay from "./PreviewOverlay";
import PreviewPane from "./PreviewPane";
import ResultMenu from "./ResultMenu";
import FilterBar, { FILTERS, type FilterId } from "../components/FilterBar";
import MenuBar, { type MenuDef } from "../components/MenuBar";
import { useConfirm } from "../components/ConfirmDialog";
import { recycleItem } from "./itemActions";
import ManageBody from "../manage/ManageBody";
import {
  search as apiSearch,
  searchCount,
  getSetting,
  setSetting,
  actionOpen,
  actionLocate,
  actionCopyPath,
  actionCopyFiles,
  pathProbe,
  appExit,
  favoriteToggle,
  scanStatus,
  scopesList,
  type FileRow,
} from "../api";
import {
  SEARCH_HISTORY_ENABLED_KEY,
  SEARCH_HISTORY_KEY,
  parseSearchHistory,
  recordSearch,
  removeSearch,
} from "../searchHistory";
import IdleHint, { TIPS } from "./IdleHint";
import { folderFilterOf, pathQueryOf, withPathFilter } from "../searchScope";
import SearchHistory from "./SearchHistory";
import type { Density } from "../types";
import { clampPaneWidth, DEFAULT_PANE_WIDTH } from "../layout";
import {
  DEFAULT_START_VIEW,
  HIDE_BROWSE_ON_EMPTY_KEY,
  LEGACY_START_LOCATION_KEY,
  START_VIEW_KEY,
  getStartViewQueryDefaults,
  resolveStartView,
  shouldHideBrowseList,
  type StartViewSetting,
} from "../startView";

const DENSITY_KEY = "ui.density";
const PREVIEW_PANE_KEY = "ui.previewPane";
const PREVIEW_PANE_WIDTH_KEY = "ui.previewPaneWidth";
// 隐藏时是否清空搜索词（隐私选项，默认关闭：打开文件返回后保留上次搜索）
const CLEAR_ON_HIDE_KEY = "ui.clearOnHide";

const FILTER_QUERY: Record<FilterId, string> = {
  all: "",
  document: "type:document",
  folder: "type:folder",
  image: "type:image",
  audio: "type:audio",
  video: "type:video",
  recent: "modified:7d",
};

const SORTS: { id: string; label: string }[] = [
  { id: "relevance", label: "相关度" },
  { id: "modified", label: "修改时间" },
  { id: "name", label: "名称" },
  { id: "size", label: "大小" },
];

export default function QuickApp() {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<FilterId>("all");
  const [sortOpen, setSortOpen] = useState(false);
  const [density, setDensity] = useState<Density>("compact");
  const [items, setItems] = useState<FileRow[]>([]);
  // 是否已收到过一次搜索结果：此前列表为空只是「还没查完」，不能显示「没有匹配的结果」
  const [stale, setStale] = useState(false);
  // 命中数还在后台统计：先显示「N+」，算完再换成精确值
  const [matchedPending, setMatchedPending] = useState(false);
  const [searched, setSearched] = useState(false);
  const [total, setTotal] = useState(0);
  // 空搜索隐藏结果时状态栏显示的全库索引总数（未取到前为 null，不显示）
  const [allTotal, setAllTotal] = useState<number | null>(null);
  // 命中总数：结果超过 200 条上限时，用它告诉用户实际命中了多少条
  const [matched, setMatched] = useState(0);
  const [elapsed, setElapsed] = useState(0);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [toast, setToast] = useState<string | null>(null);
  // 预览始终展示当前选中项，方向键切换即切换预览文件
  const [previewOpen, setPreviewOpen] = useState(false);
  // 预览窗格：搜索窗右侧常驻预览，默认关闭
  const [previewPane, setPreviewPane] = useState(false);
  const [paneWidth, setPaneWidth] = useState(DEFAULT_PANE_WIDTH);
  const paneWidthRef = useRef(DEFAULT_PANE_WIDTH);
  const areaRef = useRef<HTMLDivElement>(null);
  const [areaWidth, setAreaWidth] = useState(0);
  const [menuFor, setMenuFor] = useState<FileRow | null>(null);
  const [menuPos, setMenuPos] = useState<{ x: number; y: number } | undefined>(undefined);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsPage, setSettingsPage] = useState<string | undefined>(undefined);
  const [menubarOpen, setMenubarOpen] = useState(false);
  const [renameFor, setRenameFor] = useState<FileRow | null>(null);
  const navigatedRef = useRef(false);
  const [navigated, setNavigated] = useState(false);
  const { confirm, confirmEl } = useConfirm();
  const [hotkey, setHotkey] = useState("Alt+Q");
  const [clearOnHide, setClearOnHide] = useState(false);
  const [startView, setStartViewState] = useState<StartViewSetting>({
    ...DEFAULT_START_VIEW,
  });
  const [startViewReady, setStartViewReady] = useState(false);
  // 空搜索时是否隐藏启动列表（隐私 / 专注选项，默认关闭）
  const [hideOnEmpty, setHideOnEmpty] = useState(false);
  const [searchSort, setSearchSort] = useState("relevance");
  const [browseSortOverride, setBrowseSortOverride] = useState<string | null>(null);
  // 搜索历史（仅保存在本机，可在设置关闭或清空）
  const [history, setHistory] = useState<string[]>([]);
  const [historyEnabled, setHistoryEnabled] = useState(true);
  const [historyOpen, setHistoryOpen] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const toastTimer = useRef<number | null>(null);
  // 搜索竞态守卫：过期响应不得覆盖新结果（设计文档 §11）
  const searchGen = useRef(0);

  const notice = useCallback((message: string) => {
    setToast(message);
    if (toastTimer.current !== null) window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 2600);
  }, []);

  // 失焦处理器（依赖为空）需要读取最新值，用 ref 镜像
  const queryRef = useRef("");
  useEffect(() => {
    queryRef.current = query;
  }, [query]);
  const historyRef = useRef<string[]>([]);
  const historyEnabledRef = useRef(true);
  useEffect(() => {
    historyEnabledRef.current = historyEnabled;
  }, [historyEnabled]);

  const applyHistory = useCallback((next: string[]) => {
    historyRef.current = next;
    setHistory(next);
  }, []);

  // 记录搜索词：去重置顶后持久化（Enter 或窗口隐藏时调用）
  const recordSearchTerm = useCallback(
    (term: string) => {
      if (!historyEnabledRef.current) return;
      const next = recordSearch(historyRef.current, term);
      if (next === historyRef.current) return;
      applyHistory(next);
      setSetting(SEARCH_HISTORY_KEY, next).catch(() => undefined);
    },
    [applyHistory],
  );

  const removeHistoryTerm = useCallback(
    (term: string) => {
      applyHistory(removeSearch(historyRef.current, term));
      setSetting(SEARCH_HISTORY_KEY, historyRef.current).catch(() => undefined);
    },
    [applyHistory],
  );

  const clearHistory = useCallback(() => {
    applyHistory([]);
    setSetting(SEARCH_HISTORY_KEY, []).catch(() => undefined);
  }, [applyHistory]);

  const refreshStartView = useCallback(async () => {
    searchGen.current += 1;
    setStartViewReady(false);
    try {
      const [nextScopes, currentValue, legacyValue, hideValue] = await Promise.all([
        scopesList(),
        getSetting<unknown>(START_VIEW_KEY),
        getSetting<unknown>(LEGACY_START_LOCATION_KEY),
        getSetting<boolean>(HIDE_BROWSE_ON_EMPTY_KEY),
      ]);
      const resolved = resolveStartView(currentValue, legacyValue, nextScopes);
      setStartViewState(resolved.setting);
      setHideOnEmpty(hideValue !== null ? !!hideValue : false);
      setBrowseSortOverride(null);
      if (resolved.needsPersist) {
        setSetting(START_VIEW_KEY, resolved.setting).catch((error) =>
          notice("启动列表设置迁移失败：" + String(error)),
        );
      }
    } catch (error) {
      notice("读取启动列表设置失败：" + String(error));
    } finally {
      setStartViewReady(true);
    }
  }, [notice]);

  useEffect(() => {
    getSetting<boolean>(PREVIEW_PANE_KEY)
      .then((v) => setPreviewPane(v === true))
      .catch(() => undefined);
    getSetting<number>(PREVIEW_PANE_WIDTH_KEY)
      .then((v) => {
        if (typeof v === "number" && Number.isFinite(v)) {
          paneWidthRef.current = v;
          setPaneWidth(v);
        }
      })
      .catch(() => undefined);
    getSetting<string>(DENSITY_KEY)
      .then((v) => {
        if (v === "compact" || v === "comfortable" || v === "large") setDensity(v);
      })
      .catch(() => undefined);
    getSetting<string>("ui.hotkeyActive")
      .then((v) => {
        if (v) setHotkey(v);
      })
      .catch(() => undefined);
    getSetting<boolean>(CLEAR_ON_HIDE_KEY)
      .then((v) => setClearOnHide(!!v))
      .catch(() => undefined);
    getSetting<boolean>(SEARCH_HISTORY_ENABLED_KEY)
      .then((v) => setHistoryEnabled(v !== null ? !!v : true))
      .catch(() => undefined);
  }, []);

  // 索引位置与启动列表设置：启动与关闭设置视图后刷新
  useEffect(() => {
    if (!settingsOpen) void refreshStartView();
  }, [refreshStartView, settingsOpen]);

  // 搜索历史：启动与关闭设置视图后刷新（设置页可能已关闭或清空历史）
  const reloadHistory = useCallback(() => {
    Promise.all([
      getSetting<unknown>(SEARCH_HISTORY_KEY),
      getSetting<boolean>(SEARCH_HISTORY_ENABLED_KEY),
    ])
      .then(([raw, enabled]) => {
        applyHistory(parseSearchHistory(raw));
        setHistoryEnabled(enabled !== null ? !!enabled : true);
      })
      .catch(() => undefined);
  }, [applyHistory]);
  useEffect(() => {
    if (!settingsOpen) reloadHistory();
  }, [reloadHistory, settingsOpen]);

  // 窗口失焦（自动隐藏）时清空搜索词与浮层；重新呼出时聚焦输入框。
  const settingsOpenRef = useRef(false);
  useEffect(() => {
    settingsOpenRef.current = settingsOpen;
  }, [settingsOpen]);
  const clearOnHideRef = useRef(false);
  useEffect(() => {
    clearOnHideRef.current = clearOnHide;
  }, [clearOnHide]);

  useEffect(() => {
    const win = getCurrentWindow();
    const unlistenP = win.onFocusChanged(({ payload: focused }) => {
      if (!focused) {
        // 隐藏前记录当前搜索词（设置视图打开期间不记录，用户可能正在编辑）
        if (!settingsOpenRef.current && queryRef.current.trim()) {
          recordSearchTerm(queryRef.current);
        }
        // 仅在用户开启隐私选项时清空搜索词；设置视图打开期间一律保留（用户可能正在编辑）
        if (!settingsOpenRef.current && clearOnHideRef.current) setQuery("");
        setHistoryOpen(false);
        setPreviewOpen(false);
        setMenuFor(null);
        setSortOpen(false);
      } else if (!settingsOpenRef.current) {
        window.setTimeout(() => inputRef.current?.focus(), 60);
      }
    });
    return () => void unlistenP.then((f) => f());
  }, [recordSearchTerm]);

  const hasQuery = query.trim().length > 0;
  // 搜索框里整体是个绝对路径：列出该目录内容，Enter 直接打开该路径（用方向键选过某项后则打开所选项）
  const pathMode = pathQueryOf(query);
  const queryDefaults = getStartViewQueryDefaults(startView, hasQuery);
  // 点了筛选按钮即「按条件找文件」：跨全部启用的索引位置，默认按修改时间（最近优先）；
  // 「大文件」启动视图仍按大小。只有未选筛选的空搜索才是「浏览默认索引位置」。
  const browseDefaultSort =
    filter === "all" ? queryDefaults.sort : filter === "recent" || startView.mode !== "large" ? "modified" : "size";
  const activeSort = hasQuery ? searchSort : browseSortOverride ?? browseDefaultSort;
  const visibleSorts = hasQuery ? SORTS : SORTS.filter((option) => option.id !== "relevance");
  const browseNoScope = !hasQuery && startView.mode === "index" && startView.scopeId === null;
  // 开启「空搜索时隐藏结果」后，空搜索且未选筛选时列表区域完全空白（无索引位置时仍显示引导）
  const browseHidden = shouldHideBrowseList(hasQuery, filter, hideOnEmpty, browseNoScope);

  const changeSort = useCallback(
    (next: string) => {
      if (hasQuery) {
        setSearchSort(next);
      } else {
        setBrowseSortOverride(next === "relevance" ? null : next);
      }
    },
    [hasQuery],
  );

  // 跟踪结果区宽度：窗口缩小时窗格显示宽度自动收进可用范围（保存的宽度不变，窗口放大后恢复）
  useEffect(() => {
    const el = areaRef.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => setAreaWidth(el.clientWidth));
    ro.observe(el);
    setAreaWidth(el.clientWidth);
    return () => ro.disconnect();
  }, [previewPane, browseHidden]);

  /** 拖拽分隔条调整预览窗格宽度：向左拖变宽；松手后持久化，双击恢复默认。 */
  const startPaneResize = useCallback((event: React.MouseEvent) => {
    event.preventDefault();
    const area = areaRef.current;
    if (!area) return;
    const startX = event.clientX;
    const startWidth = paneWidthRef.current;
    document.body.classList.add("col-resizing");
    const onMove = (e: MouseEvent) => {
      const next = clampPaneWidth(startWidth + (startX - e.clientX), area.clientWidth);
      paneWidthRef.current = next;
      setPaneWidth(next);
    };
    const onUp = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      document.body.classList.remove("col-resizing");
      setSetting(PREVIEW_PANE_WIDTH_KEY, paneWidthRef.current).catch(() => undefined);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }, []);

  const resetPaneWidth = useCallback(() => {
    paneWidthRef.current = DEFAULT_PANE_WIDTH;
    setPaneWidth(DEFAULT_PANE_WIDTH);
    setSetting(PREVIEW_PANE_WIDTH_KEY, DEFAULT_PANE_WIDTH).catch(() => undefined);
  }, []);

  const togglePreviewPane = useCallback(() => {
    setPreviewPane((on) => {
      setSetting(PREVIEW_PANE_KEY, !on).catch(() => undefined);
      return !on;
    });
  }, []);

  const changeDensity = useCallback((next: Density) => {
    setDensity(next);
    setSetting(DENSITY_KEY, next).catch(() => undefined);
  }, []);

  const runSearch = useCallback(() => {
    if (!startViewReady) return;

    const q = [pathMode ? `path:"${pathMode}"` : query, FILTER_QUERY[filter]].filter(Boolean).join(" ");
    // 路径模式没有文本相关度，相关度排序改按名称
    const sortForQuery = pathMode && activeSort === "relevance" ? "name" : activeSort;
    const scopeId = filter === "all" ? queryDefaults.scopeId : undefined;
    if (browseNoScope || browseHidden) {
      const hiddenGen = ++searchGen.current;
      setItems([]);
      setMatched(0);
      setMatchedPending(false);
      setElapsed(0);
      setTotal(0);
      if (browseHidden) {
        // 列表留白，但状态栏仍显示索引总数：不带位置取全库总数（只取 1 条，后端有 10 秒缓存）。
        // 带位置时后端只数该位置根目录第一层，数字会远小于实际索引量，看起来像异常。
        apiSearch("", undefined, 1, undefined)
          .then((response) => {
            if (hiddenGen === searchGen.current) setAllTotal(response.total);
          })
          .catch(() => undefined);
      }
      return;
    }

    const gen = ++searchGen.current;
    // 查询超过 150ms 才让旧结果变淡：快速查询不闪烁
    const staleTimer = window.setTimeout(() => {
      if (gen === searchGen.current) setStale(true);
    }, 150);
    apiSearch(q, sortForQuery === "relevance" ? undefined : sortForQuery, 200, scopeId)
      .then((response) => {
        window.clearTimeout(staleTimer);
        if (gen !== searchGen.current) return;
        setStale(false);
        setItems(response.items);
        setSearched(true);
        setMatched(response.matched);
        setMatchedPending(response.matchedPending === true);
        if (response.matchedPending) {
          searchCount(q, scopeId)
            .then((n) => {
              if (gen !== searchGen.current) return;
              setMatched(n);
              setMatchedPending(false);
            })
            .catch(() => undefined);
        }
        setTotal(response.total);
        setElapsed(response.elapsedMs);
      })
      .catch((error) => {
        window.clearTimeout(staleTimer);
        if (gen !== searchGen.current) return;
        setStale(false);
        notice("搜索失败：" + String(error));
      });
  }, [
    activeSort,
    browseHidden,
    filter,
    hasQuery,
    notice,
    pathMode,
    query,
    queryDefaults.scopeId,
    startView.mode,
    startView.scopeId,
    startViewReady,
  ]);

  useEffect(() => {
    navigatedRef.current = false;
    setNavigated(false);
  }, [query]);

  // 搜索：50ms 防抖（设计文档 §3.2）
  useEffect(() => {
    const t = window.setTimeout(runSearch, 50);
    return () => window.clearTimeout(t);
  }, [runSearch]);

  // 从设置返回：索引范围的启用状态可能变了，重新查询让命中数与「索引 N 条」同步
  useEffect(() => {
    if (!settingsOpen) runSearch();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settingsOpen]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [activeSort, filter, query, startView.mode, startView.scopeId]);

  // 有输入词时自动收起历史下拉；展开只由搜索框内的下拉箭头控制，默认不自动弹出
  useEffect(() => {
    if (hasQuery) setHistoryOpen(false);
  }, [hasQuery]);

  // 索引进度仅在设置内查看；扫描结束后刷新索引总数
  const wasScanning = useRef(false);
  useEffect(() => {
    let stop = false;
    const poll = () => {
      scanStatus()
        .then((s) => {
          if (stop) return;
          if (wasScanning.current && !s) runSearch();
          wasScanning.current = !!s;
        })
        .catch(() => undefined);
    };
    poll();
    const t = window.setInterval(poll, 3000);
    return () => {
      stop = true;
      window.clearInterval(t);
    };
  }, [runSearch]);

  const onKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>) => {
      // 中文输入法合成期间（选候选词）不拦截任何按键
      if (event.nativeEvent.isComposing) return;
      if (settingsOpen) {
        if (event.key === "Escape") setSettingsOpen(false);
        return;
      }
      if (menuFor || renameFor || menubarOpen) {
        if (event.key === "Escape" && !menubarOpen) {
          setMenuFor(null);
          setRenameFor(null);
        }
        return;
      }
      if (previewOpen) {
        if (event.key === "Escape" || event.key === " ") {
          event.preventDefault();
          setPreviewOpen(false);
        } else if (event.key === "ArrowRight" || event.key === "ArrowDown") {
          event.preventDefault();
          setSelectedIndex((index) => Math.min(index + 1, items.length - 1));
        } else if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
          event.preventDefault();
          setSelectedIndex((index) => Math.max(index - 1, 0));
        } else if (event.key === "Enter") {
          event.preventDefault();
          const it = items[selectedIndex];
          if (it) {
            actionOpen(it.path)
              .then((m) => notice(m))
              .catch((e) => notice(String(e)));
          }
        }
        return;
      }
      const inputEl = inputRef.current;
      const typing = document.activeElement === inputEl;
      const hasTextSelection =
        typing &&
        inputEl !== null &&
        inputEl.selectionStart !== null &&
        inputEl.selectionEnd !== null &&
        inputEl.selectionStart !== inputEl.selectionEnd;

      if (event.altKey && event.key.toLowerCase() === "p") {
        event.preventDefault();
        togglePreviewPane();
        return;
      }
      if (event.key === "Escape" && historyOpen) {
        event.preventDefault();
        setHistoryOpen(false);
        return;
      }
      if (event.key === "ArrowDown" && items.length > 0) {
        event.preventDefault();
        navigatedRef.current = true;
        setNavigated(true);
        setSelectedIndex((index) => Math.min(index + 1, items.length - 1));
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        navigatedRef.current = true;
        setNavigated(true);
        setSelectedIndex((index) => Math.max(index - 1, 0));
      } else if (event.key === "Enter" && !hasTextSelection) {
        event.preventDefault();
        // 明确的回车动作视为一次搜索提交，记录进历史（去重由 recordSearch 处理）
        if (hasQuery) recordSearchTerm(query);
        // 直接输入了路径：先确认存在，再隐藏窗口并用系统默认方式打开（目录会在资源管理器中打开）
        if (pathMode && !navigatedRef.current && !event.ctrlKey) {
          pathProbe(pathMode)
            .then((kind) => {
              if (!kind) {
                notice("路径不存在：" + pathMode);
                return;
              }
              void getCurrentWindow().hide();
              return actionOpen(pathMode).then(() => notice("已打开 " + pathMode));
            })
            .catch((e) => notice(String(e)));
          return;
        }
        const it = items[selectedIndex];
        if (!it) return;
        if (event.ctrlKey) {
          actionLocate(it.path)
            .then((m) => notice(m))
            .catch((e) => notice(String(e)));
        } else {
          const target = it.path;
          void getCurrentWindow().hide();
          actionOpen(target)
            .then(() => notice("已打开 " + target.split("\\").pop()))
            .catch((e) => notice(String(e)));
        }
      } else if (event.key === " " && !typing) {
        event.preventDefault();
        const it = items[selectedIndex];
        if (it) setPreviewOpen(true);
      } else if (event.key === "Escape") {
        event.preventDefault();
        if (query) {
          setQuery("");
        } else {
          void getCurrentWindow().hide();
        }
      } else if (event.key === "l" && (event.ctrlKey || event.metaKey)) {
        event.preventDefault();
        inputRef.current?.focus();
        inputRef.current?.select();
      } else if (event.key === "c" && (event.ctrlKey || event.metaKey) && !hasTextSelection) {
        const it = items[selectedIndex];
        if (!it) return;
        event.preventDefault();
        if (event.shiftKey) {
          actionCopyFiles([it.path])
            .then((m) => notice(m))
            .catch((e) => notice(String(e)));
        } else {
          actionCopyPath(it.path, "full")
            .then((m) => notice(m))
            .catch((e) => notice(String(e)));
        }
      }
    },
    [items, selectedIndex, query, hasQuery, historyOpen, previewOpen, menuFor, renameFor, menubarOpen, settingsOpen, notice, recordSearchTerm, togglePreviewPane, pathMode]
  );

  const openSettings = (page?: string) => {
    setSettingsPage(page);
    setSettingsOpen(true);
  };
  const focusInput = () => inputRef.current?.focus();

  // ---- 菜单栏：对「当前选中项」的操作与现有右键菜单 / 快捷键共用同一批动作 ----
  const selected = browseHidden ? null : items[selectedIndex] ?? null;
  const noSel = selected === null;
  const noteResult = (p: Promise<string>) => p.then((m) => notice(m)).catch((e) => notice(String(e)));
  const menus: MenuDef[] = [
    {
      id: "file",
      label: "文件(F)",
      accel: "f",
      entries: [
        {
          kind: "item",
          label: "打开",
          hint: "Enter",
          disabled: noSel,
          onSelect: () => {
            if (!selected) return;
            void getCurrentWindow().hide();
            noteResult(actionOpen(selected.path));
          },
        },
        {
          kind: "item",
          label: "在文件夹中定位",
          hint: "Ctrl+Enter",
          disabled: noSel,
          onSelect: () => selected && noteResult(actionLocate(selected.path)),
        },
        { kind: "sep" },
        {
          kind: "item",
          label: "复制完整路径",
          hint: "Ctrl+C",
          disabled: noSel,
          onSelect: () => selected && noteResult(actionCopyPath(selected.path, "full")),
        },
        {
          kind: "item",
          label: selected?.isDir ? "复制文件夹" : "复制文件",
          hint: "Ctrl+Shift+C",
          disabled: noSel,
          onSelect: () => selected && noteResult(actionCopyFiles([selected.path])),
        },
        { kind: "sep" },
        { kind: "item", label: "重命名…", disabled: noSel, onSelect: () => selected && setRenameFor(selected) },
        {
          kind: "item",
          label: selected?.favorite ? "取消收藏" : "收藏",
          disabled: noSel,
          onSelect: () => {
            if (!selected) return;
            favoriteToggle(selected.fileId)
              .then(() => {
                notice(selected.favorite ? "已取消收藏" : "已收藏");
                runSearch();
              })
              .catch((e) => notice(String(e)));
          },
        },
        {
          kind: "item",
          label: "移至回收站",
          danger: true,
          disabled: noSel,
          onSelect: () => {
            if (!selected) return;
            recycleItem(selected, confirm)
              .then((m) => {
                if (m === null) return;
                notice(m);
                runSearch();
              })
              .catch((e) => notice(String(e)));
          },
        },
        { kind: "sep" },
        { kind: "item", label: "隐藏窗口", hint: "Esc", onSelect: () => void getCurrentWindow().hide() },
        { kind: "item", label: "退出 FindFlow", onSelect: () => appExit().catch((e) => notice(String(e))) },
      ],
    },
    {
      id: "edit",
      label: "编辑(E)",
      accel: "e",
      entries: [
        { kind: "title", label: "搜索框" },
        {
          kind: "item",
          label: "剪切",
          hint: "Ctrl+X",
          onSelect: () => {
            focusInput();
            document.execCommand("cut");
          },
        },
        {
          kind: "item",
          label: "复制",
          hint: "Ctrl+C",
          onSelect: () => {
            focusInput();
            document.execCommand("copy");
          },
        },
        {
          kind: "item",
          label: "粘贴",
          hint: "Ctrl+V",
          onSelect: () => {
            navigator.clipboard
              .readText()
              .then((text) => {
                const el = inputRef.current;
                if (!el) return;
                el.focus();
                el.setRangeText(text, el.selectionStart ?? el.value.length, el.selectionEnd ?? el.value.length, "end");
                setQuery(el.value);
              })
              .catch(() => notice("无法读取剪贴板"));
          },
        },
        {
          kind: "item",
          label: "全选",
          hint: "Ctrl+A",
          onSelect: () => {
            focusInput();
            inputRef.current?.select();
          },
        },
        { kind: "sep" },
        {
          kind: "item",
          label: "清空搜索框",
          hint: "Esc",
          disabled: !query,
          onSelect: () => {
            setQuery("");
            focusInput();
          },
        },
        { kind: "item", label: "聚焦搜索框", hint: "Ctrl+L", onSelect: () => { focusInput(); inputRef.current?.select(); } },
      ],
    },
    {
      id: "view",
      label: "视图(V)",
      accel: "v",
      entries: [
        {
          kind: "item",
          label: "预览窗格",
          hint: "Alt+P",
          checked: previewPane,
          onSelect: () => {
            togglePreviewPane();
            focusInput();
          },
        },
        {
          kind: "item",
          label: "预览所选文件",
          hint: "Space",
          disabled: noSel,
          onSelect: () => selected && setPreviewOpen(true),
        },
        { kind: "sep" },
        { kind: "title", label: "显示密度" },
        ...(["compact", "comfortable", "large"] as Density[]).map((d) => ({
          kind: "item" as const,
          label: d === "compact" ? "紧凑" : d === "comfortable" ? "舒适" : "大图标",
          checked: density === d,
          onSelect: () => {
            changeDensity(d);
            focusInput();
          },
        })),
        { kind: "sep" },
        { kind: "title", label: "排序方式" },
        ...visibleSorts.map((option) => ({
          kind: "item" as const,
          label: option.label,
          checked: activeSort === option.id,
          onSelect: () => {
            changeSort(option.id);
            focusInput();
          },
        })),
      ],
    },
    {
      id: "search",
      label: "搜索(S)",
      accel: "s",
      entries: [
        { kind: "title", label: "类型筛选" },
        ...FILTERS.map((f) => ({
          kind: "item" as const,
          label: f.label,
          checked: filter === f.id,
          onSelect: () => {
            setFilter(f.id);
            focusInput();
          },
        })),
        { kind: "sep" },
        { kind: "title", label: "语法示例（填入搜索框）" },
        ...TIPS.map((tip) => ({
          kind: "item" as const,
          label: tip.query,
          hint: tip.desc,
          onSelect: () => {
            setQuery(tip.query);
            focusInput();
          },
        })),
        { kind: "sep" },
        {
          kind: "item",
          label: "仅搜索所选项所在文件夹",
          disabled: !selected || folderFilterOf(selected.path) === null,
          onSelect: () => {
            const filter = selected ? folderFilterOf(selected.path) : null;
            if (!filter) return;
            setQuery(withPathFilter(query, filter));
            focusInput();
          },
        },
        {
          kind: "item",
          label: "清除文件夹限定",
          disabled: !/(^|\s)path:/i.test(query),
          onSelect: () => {
            setQuery(query.replace(/(^|\s)path:("[^"]*"|\S+)/gi, "").trim());
            focusInput();
          },
        },
        { kind: "sep" },
        {
          kind: "item",
          label: "搜索历史",
          disabled: !historyEnabled || history.length === 0,
          onSelect: () => {
            focusInput();
            setHistoryOpen(true);
          },
        },
        {
          kind: "item",
          label: "清空搜索历史",
          disabled: history.length === 0,
          onSelect: () => {
            clearHistory();
            focusInput();
          },
        },
      ],
    },
    {
      id: "tools",
      label: "工具(T)",
      accel: "t",
      entries: [
        { kind: "item", label: "设置…", onSelect: () => openSettings() },
        { kind: "sep" },
        { kind: "item", label: "索引范围…", onSelect: () => openSettings("index") },
        { kind: "item", label: "任务…", onSelect: () => openSettings("tasks") },
        { kind: "item", label: "批量处理…", onSelect: () => openSettings("batch") },
        { kind: "item", label: "空间分析…", onSelect: () => openSettings("space") },
        { kind: "item", label: "收藏与标签…", onSelect: () => openSettings("library") },
      ],
    },
    {
      id: "help",
      label: "帮助(H)",
      accel: "h",
      entries: [
        { kind: "item", label: "快捷键…", onSelect: () => openSettings("hotkey") },
        { kind: "item", label: "关于 FindFlow", onSelect: () => openSettings("about") },
      ],
    },
  ];

  return (
    <div className="app" onKeyDown={onKeyDown}>
      {confirmEl}
      <TitleBar onOpenSettings={() => openSettings()} />
      {settingsOpen ? (
        <>
          <div className="settings-top">
            <button className="settings-back" onClick={() => setSettingsOpen(false)}>
              <ArrowLeftIcon />
              返回搜索
              <kbd>Esc</kbd>
            </button>
          </div>
          <ManageBody notice={notice} initialPage={settingsPage} key={settingsPage ?? "default"} />
        </>
      ) : (
        <>
          <MenuBar menus={menus} onRestoreFocus={focusInput} onOpenChange={setMenubarOpen} />
          <div className="searchbar">
            <span className="searchbar-icon">
              <SearchIcon />
            </span>
            <input
              ref={inputRef}
              className="searchbar-input"
              placeholder="搜索文件名、路径，或语法筛选（ext:docx size:>1GB modified:7d *.pdf）"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              autoFocus
            />
            {query ? (
              <button
                type="button"
                className="searchbar-clear"
                aria-label="清除搜索词"
                title="清除搜索词"
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => {
                  setQuery("");
                  inputRef.current?.focus();
                }}
              >
                <CloseIcon size={14} />
              </button>
            ) : null}
            {historyEnabled && history.length > 0 ? (
              <button
                type="button"
                className={"searchbar-history" + (historyOpen ? " open" : "")}
                aria-label={historyOpen ? "收起搜索历史" : "展开搜索历史"}
                aria-expanded={historyOpen}
                title="搜索历史"
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => setHistoryOpen((value) => !value)}
              >
                <ChevronDownIcon size={16} />
              </button>
            ) : null}
            {historyOpen && historyEnabled ? (
              <SearchHistory
                items={history}
                onPick={(term) => {
                  setQuery(term);
                  setHistoryOpen(false);
                  inputRef.current?.focus();
                }}
                onRemove={removeHistoryTerm}
                onClear={() => {
                  clearHistory();
                  setHistoryOpen(false);
                }}
                onClose={() => setHistoryOpen(false)}
              />
            ) : null}
            {browseHidden ? null : (
              <div
                className="sort-wrap"
                onBlur={(e) => {
                  if (!e.currentTarget.contains(e.relatedTarget as Node)) setSortOpen(false);
                }}
              >
                <button className="sort-btn" onClick={() => setSortOpen((value) => !value)}>
                  {SORTS.find((option) => option.id === activeSort)?.label} ▾
                </button>
                {sortOpen ? (
                  <div className="sort-menu">
                    {visibleSorts.map((option) => (
                      <button
                        key={option.id}
                        className={option.id === activeSort ? "active" : ""}
                        onClick={() => {
                          changeSort(option.id);
                          setSortOpen(false);
                        }}
                      >
                        {option.label}
                      </button>
                    ))}
                  </div>
                ) : null}
              </div>
            )}
            <span className="searchbar-kbd">{hotkey}</span>
          </div>
          <FilterBar
            filter={filter}
            onFilterChange={setFilter}
            density={density}
            onDensityChange={changeDensity}
            count={browseHidden ? undefined : matched}
            limited={items.length >= 200}
            filtered={hasQuery || filter !== "all"}
            shown={items.length}
            countPending={matchedPending}
            previewPane={previewPane}
            onTogglePreviewPane={togglePreviewPane}
          />
          {browseHidden ? (
            <IdleHint
              onPick={(term) => {
                setQuery(term);
                inputRef.current?.focus();
              }}
            />
          ) : (
            <div ref={areaRef} className={"result-area" + (previewPane ? " pane-on" : "")}>
            <ResultList
              items={items}
              density={density}
              selectedIndex={selectedIndex}
              sort={activeSort}
              onSort={changeSort}
              onSelect={setSelectedIndex}
              onNotice={notice}
              onMenu={(item, pos) => {
                setMenuPos(pos);
                setMenuFor(item);
              }}
              onOpenSettings={() => setSettingsOpen(true)}
              query={query}
              stale={stale}
              emptyReason={
                browseNoScope ? "no-scope" : !searched ? "loading" : filter !== "all" ? "filtered" : undefined
              }
            />
            {previewPane ? (
              <>
                <div
                  className="pane-splitter"
                  role="separator"
                  aria-orientation="vertical"
                  title="拖拽调整预览宽度，双击恢复默认"
                  onMouseDown={startPaneResize}
                  onDoubleClick={resetPaneWidth}
                />
                <PreviewPane item={items[selectedIndex] ?? null} onNotice={notice} width={areaWidth > 0 ? clampPaneWidth(paneWidth, areaWidth) : paneWidth} />
              </>
            ) : null}
            </div>
          )}
          <FooterBar
            item={browseHidden ? null : items[selectedIndex] ?? null}
            total={browseNoScope ? undefined : browseHidden ? allTotal ?? undefined : total}
            elapsed={browseHidden || browseNoScope ? undefined : elapsed}
            openHint={pathMode && !navigated ? pathMode : undefined}
          />
        </>
      )}
      {menuFor ? (
        <ResultMenu
          item={menuFor}
          pos={menuPos}
          onClose={() => setMenuFor(null)}
          onNotice={notice}
          onRefresh={runSearch}
        />
      ) : null}
      {renameFor ? (
        <ResultMenu
          item={renameFor}
          initial="rename"
          onClose={() => setRenameFor(null)}
          onNotice={notice}
          onRefresh={runSearch}
        />
      ) : null}
      {previewOpen && items[selectedIndex] ? (
        <PreviewOverlay item={items[selectedIndex]} onClose={() => setPreviewOpen(false)} onNotice={notice} />
      ) : null}
      {toast ? <Toast message={toast} /> : null}
    </div>
  );
}
