import { useCallback, useEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import TitleBar from "../components/TitleBar";
import ResultList from "../components/ResultList";
import FooterBar from "../components/FooterBar";
import { Toast } from "../components/Toast";
import { ArrowLeftIcon, ChevronDownIcon, CloseIcon, SearchIcon } from "../components/Icons";
import PreviewOverlay from "./PreviewOverlay";
import ResultMenu from "./ResultMenu";
import FilterBar, { type FilterId } from "../components/FilterBar";
import ManageBody from "../manage/ManageBody";
import {
  search as apiSearch,
  getSetting,
  setSetting,
  actionOpen,
  actionLocate,
  actionCopyPath,
  actionCopyFiles,
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
import SearchHistory from "./SearchHistory";
import type { Density } from "../types";
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
  const [menuFor, setMenuFor] = useState<FileRow | null>(null);
  const [menuPos, setMenuPos] = useState<{ x: number; y: number } | undefined>(undefined);
  const [settingsOpen, setSettingsOpen] = useState(false);
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
  const queryDefaults = getStartViewQueryDefaults(startView, hasQuery);
  const browseDefaultSort = filter === "recent" ? "modified" : queryDefaults.sort;
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

  const changeDensity = useCallback((next: Density) => {
    setDensity(next);
    setSetting(DENSITY_KEY, next).catch(() => undefined);
  }, []);

  const runSearch = useCallback(() => {
    if (!startViewReady) return;

    const q = [query, FILTER_QUERY[filter]].filter(Boolean).join(" ");
    const scopeId = queryDefaults.scopeId;
    if (browseNoScope || browseHidden) {
      const hiddenGen = ++searchGen.current;
      setItems([]);
      setMatched(0);
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
    apiSearch(q, activeSort === "relevance" ? undefined : activeSort, 200, scopeId)
      .then((response) => {
        if (gen !== searchGen.current) return;
        setItems(response.items);
        setSearched(true);
        setMatched(response.matched);
        setTotal(response.total);
        setElapsed(response.elapsedMs);
      })
      .catch((error) => {
        if (gen !== searchGen.current) return;
        notice("搜索失败：" + String(error));
      });
  }, [
    activeSort,
    browseHidden,
    filter,
    hasQuery,
    notice,
    query,
    queryDefaults.scopeId,
    startView.mode,
    startView.scopeId,
    startViewReady,
  ]);

  // 搜索：50ms 防抖（设计文档 §3.2）
  useEffect(() => {
    const t = window.setTimeout(runSearch, 50);
    return () => window.clearTimeout(t);
  }, [runSearch]);

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
      if (menuFor) {
        if (event.key === "Escape") setMenuFor(null);
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

      if (event.key === "Escape" && historyOpen) {
        event.preventDefault();
        setHistoryOpen(false);
        return;
      }
      if (event.key === "ArrowDown" && items.length > 0) {
        event.preventDefault();
        setSelectedIndex((index) => Math.min(index + 1, items.length - 1));
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        setSelectedIndex((index) => Math.max(index - 1, 0));
      } else if (event.key === "Enter" && !hasTextSelection) {
        event.preventDefault();
        // 明确的回车动作视为一次搜索提交，记录进历史（去重由 recordSearch 处理）
        if (hasQuery) recordSearchTerm(query);
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
    [items, selectedIndex, query, hasQuery, historyOpen, previewOpen, menuFor, settingsOpen, notice, recordSearchTerm]
  );

  return (
    <div className="app" onKeyDown={onKeyDown}>
      <TitleBar onOpenSettings={() => setSettingsOpen(true)} />
      {settingsOpen ? (
        <>
          <div className="settings-top">
            <button className="settings-back" onClick={() => setSettingsOpen(false)}>
              <ArrowLeftIcon />
              返回搜索
              <kbd>Esc</kbd>
            </button>
          </div>
          <ManageBody notice={notice} />
        </>
      ) : (
        <>
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
          />
          {browseHidden ? null : (
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
              emptyReason={
                browseNoScope ? "no-scope" : !searched ? "loading" : filter !== "all" ? "filtered" : undefined
              }
            />
          )}
          <FooterBar
            item={browseHidden ? null : items[selectedIndex] ?? null}
            total={browseNoScope ? undefined : browseHidden ? allTotal ?? undefined : total}
            elapsed={browseHidden || browseNoScope ? undefined : elapsed}
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
      {previewOpen && items[selectedIndex] ? (
        <PreviewOverlay item={items[selectedIndex]} onClose={() => setPreviewOpen(false)} onNotice={notice} />
      ) : null}
      {toast ? <Toast message={toast} /> : null}
    </div>
  );
}
