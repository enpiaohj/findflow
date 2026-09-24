import { useCallback, useEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import TitleBar from "../components/TitleBar";
import ResultList from "../components/ResultList";
import FooterBar from "../components/FooterBar";
import { Toast } from "../components/Toast";
import { SearchIcon } from "../components/Icons";
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
  type FileRow,
} from "../api";
import type { Density } from "../types";

const DENSITY_KEY = "ui.density";

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
  const [previewPath, setPreviewPath] = useState<string | null>(null);
  const [menuFor, setMenuFor] = useState<FileRow | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
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
  }, []);

  // 窗口失焦（自动隐藏）时清空搜索词与浮层；重新呼出时聚焦输入框。
  const settingsOpenRef = useRef(false);
  useEffect(() => {
    settingsOpenRef.current = settingsOpen;
  }, [settingsOpen]);

  useEffect(() => {
    const win = getCurrentWindow();
    const unlistenP = win.onFocusChanged(({ payload: focused }) => {
      if (!focused) {
        // 设置视图打开期间不清词（用户可能正在编辑）；普通搜索态按 FR01 清空敏感词
        if (!settingsOpenRef.current) setQuery("");
        setPreviewPath(null);
        setMenuFor(null);
        setSortOpen(false);
      } else if (!settingsOpenRef.current) {
        window.setTimeout(() => inputRef.current?.focus(), 60);
      }
    });
    return () => void unlistenP.then((f) => f());
  }, []);

  // 设置视图打开期间暂停“失焦自动隐藏”，避免选目录等系统对话框打断流程
  useEffect(() => {
    setSetting("ui.settingsOpen", settingsOpen).catch(() => undefined);
  }, [settingsOpen]);

  const changeDensity = useCallback((next: Density) => {
    setDensity(next);
    setSetting(DENSITY_KEY, next).catch(() => undefined);
  }, []);

  const runSearch = useCallback(() => {
    const q = [query, FILTER_QUERY[filter]].filter(Boolean).join(" ");
    const gen = ++searchGen.current;
    apiSearch(q, sort === "relevance" ? undefined : sort, 200)
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
  }, [query, filter, sort, notice]);

  // 搜索：50ms 防抖（设计文档 §3.2）
  useEffect(() => {
    const t = window.setTimeout(runSearch, 50);
    return () => window.clearTimeout(t);
  }, [runSearch]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query, filter, sort]);

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
      if (previewPath) {
        if (event.key === "Escape" || event.key === " ") {
          event.preventDefault();
          setPreviewPath(null);
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
        if (it) setPreviewPath(it.path);
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
    [items, selectedIndex, query, previewPath, menuFor, settingsOpen, notice]
  );

  return (
    <div className="app" onKeyDown={onKeyDown}>
      <TitleBar onOpenSettings={() => setSettingsOpen(true)} />
      {settingsOpen ? (
        <>
          <div className="settings-top">
            <b>设置</b>
            <button className="settings-back" onClick={() => setSettingsOpen(false)}>
              返回搜索（Esc）
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
            <span className="searchbar-kbd">Alt + Space</span>
          </div>
          <FilterBar
            filter={filter}
            onFilterChange={setFilter}
            density={density}
            onDensityChange={changeDensity}
            count={items.length}
          />
          <div className="result-status">
            {`共 ${items.length} 项 · 索引 ${total} 条 · ${elapsed} ms`}
          </div>
          <ResultList
            items={items}
            density={density}
            selectedIndex={selectedIndex}
            onSelect={setSelectedIndex}
            onNotice={notice}
            onOpen={(item) => setPreviewPath(item.path)}
            onMenu={(item) => setMenuFor(item)}
            onOpenSettings={() => setSettingsOpen(true)}
          />
          <FooterBar />
        </>
      )}
      {menuFor ? (
        <ResultMenu
          item={menuFor}
          onClose={() => setMenuFor(null)}
          onNotice={notice}
          onRefresh={runSearch}
        />
      ) : null}
      {previewPath ? (
        <PreviewOverlay path={previewPath} onClose={() => setPreviewPath(null)} onNotice={notice} />
      ) : null}
      {toast ? <Toast message={toast} /> : null}
    </div>
  );
}
