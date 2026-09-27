import { useCallback, useEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import TitleBar from "../components/TitleBar";
import ResultList from "../components/ResultList";
import FooterBar from "../components/FooterBar";
import { Toast } from "../components/Toast";
import { ArrowLeftIcon, SearchIcon } from "../components/Icons";
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
  type ScopeInfo,
} from "../api";
import type { Density } from "../types";

const DENSITY_KEY = "ui.density";
// 隐藏时是否清空搜索词（隐私选项，默认关闭：打开文件返回后保留上次搜索）
const CLEAR_ON_HIDE_KEY = "ui.clearOnHide";
// 启动时显示的位置："all" 或索引范围 scopeId（设置「启动视图」控制，搜索窗内可临时切换）
const START_LOCATION_KEY = "ui.startLocation";

const FILTER_QUERY: Record<FilterId, string> = {
  all: "",
  document: "type:document",
  folder: "type:folder",
  image: "type:image",
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
  const [sort, setSort] = useState("relevance");
  const [sortOpen, setSortOpen] = useState(false);
  const [density, setDensity] = useState<Density>("compact");
  const [items, setItems] = useState<FileRow[]>([]);
  const [total, setTotal] = useState(0);
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
  // 位置筛选：0 = 所有位置，>0 = 指定索引范围 scopeId
  const [scopes, setScopes] = useState<ScopeInfo[]>([]);
  const [locationId, setLocationId] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const toastTimer = useRef<number | null>(null);
  // 搜索竞态守卫：过期响应不得覆盖新结果（设计文档 §11）
  const searchGen = useRef(0);

  const notice = useCallback((message: string) => {
    setToast(message);
    if (toastTimer.current !== null) window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 2600);
  }, []);

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
    getSetting<string | number>(START_LOCATION_KEY)
      .then((v) => {
        if (typeof v === "number" && v > 0) setLocationId(v);
      })
      .catch(() => undefined);
  }, []);

  // 索引位置列表：启动与关闭设置视图后刷新
  useEffect(() => {
    if (!settingsOpen) scopesList().then(setScopes).catch(() => undefined);
  }, [settingsOpen]);

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
        // 仅在用户开启隐私选项时清空搜索词；设置视图打开期间一律保留（用户可能正在编辑）
        if (!settingsOpenRef.current && clearOnHideRef.current) setQuery("");
        setPreviewOpen(false);
        setMenuFor(null);
        setSortOpen(false);
      } else if (!settingsOpenRef.current) {
        window.setTimeout(() => inputRef.current?.focus(), 60);
      }
    });
    return () => void unlistenP.then((f) => f());
  }, []);

  // 启动位置已被移除 / 停用时回退为所有位置
  const activeLocation = scopes.some((s) => s.scopeId === locationId && s.enabled)
    ? locationId
    : 0;

  const changeDensity = useCallback((next: Density) => {
    setDensity(next);
    setSetting(DENSITY_KEY, next).catch(() => undefined);
  }, []);

  const runSearch = useCallback(() => {
    const q = [query, FILTER_QUERY[filter]].filter(Boolean).join(" ");
    // 关键词搜索始终覆盖全部索引位置；位置仅决定空查询时浏览列表显示的范围
    const browseScope = query.trim() ? 0 : activeLocation;
    const gen = ++searchGen.current;
    apiSearch(q, sort === "relevance" ? undefined : sort, 200, browseScope || undefined)
      .then((r) => {
        if (gen !== searchGen.current) return;
        setItems(r.items);
        setTotal(r.total);
        setElapsed(r.elapsedMs);
      })
      .catch((e) => {
        if (gen !== searchGen.current) return;
        notice("搜索失败：" + String(e));
      });
  }, [query, filter, sort, activeLocation, notice]);

  // 搜索：50ms 防抖（设计文档 §3.2）
  useEffect(() => {
    const t = window.setTimeout(runSearch, 50);
    return () => window.clearTimeout(t);
  }, [runSearch]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query, filter, sort, activeLocation]);

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

      if (event.key === "ArrowDown" && items.length > 0) {
        event.preventDefault();
        setSelectedIndex((index) => Math.min(index + 1, items.length - 1));
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        setSelectedIndex((index) => Math.max(index - 1, 0));
      } else if (event.key === "Enter" && !hasTextSelection) {
        event.preventDefault();
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
    [items, selectedIndex, query, previewOpen, menuFor, settingsOpen, notice]
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
              placeholder="搜索文件名、路径，或语法筛选（ext:docx size:>1GB modified:7d）"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              autoFocus
            />
            <div
              className="sort-wrap"
              onBlur={(e) => {
                if (!e.currentTarget.contains(e.relatedTarget as Node)) setSortOpen(false);
              }}
            >
              <button className="sort-btn" onClick={() => setSortOpen((v) => !v)}>
                {SORTS.find((s) => s.id === sort)?.label} ▾
              </button>
              {sortOpen ? (
                <div className="sort-menu">
                  {SORTS.map((s) => (
                    <button
                      key={s.id}
                      className={s.id === sort ? "active" : ""}
                      onClick={() => {
                        setSort(s.id);
                        setSortOpen(false);
                      }}
                    >
                      {s.label}
                    </button>
                  ))}
                </div>
              ) : null}
            </div>
            <span className="searchbar-kbd">{hotkey}</span>
          </div>
          <FilterBar
            filter={filter}
            onFilterChange={setFilter}
            density={density}
            onDensityChange={changeDensity}
            count={items.length}
            limited={items.length >= 200}
            scopes={scopes}
            locationId={activeLocation}
            onLocationChange={setLocationId}
            locationDisabled={!!query.trim()}
          />
          <div className="result-status">
            {`索引 ${total.toLocaleString()} 条 · 用时 ${elapsed} ms`}
          </div>
          <ResultList
            items={items}
            density={density}
            selectedIndex={selectedIndex}
            sort={sort}
            onSort={setSort}
            onSelect={setSelectedIndex}
            onNotice={notice}
            onMenu={(item, pos) => {
              setMenuPos(pos);
              setMenuFor(item);
            }}
            onOpenSettings={() => setSettingsOpen(true)}
          />
          <FooterBar />
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
