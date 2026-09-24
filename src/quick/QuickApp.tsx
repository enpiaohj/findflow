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
import { search as apiSearch, getSetting, setSetting, type FileRow } from "../api";
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
  const inputRef = useRef<HTMLInputElement>(null);
  const toastTimer = useRef<number | null>(null);

  useEffect(() => {
    getSetting<string>(DENSITY_KEY)
      .then((v) => {
        if (v === "compact" || v === "comfortable" || v === "large") setDensity(v);
      })
      .catch(() => undefined);
  }, []);

  const notice = useCallback((message: string) => {
    setToast(message);
    if (toastTimer.current !== null) window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 2400);
  }, []);

  const changeDensity = useCallback((next: Density) => {
    setDensity(next);
    setSetting(DENSITY_KEY, next).catch(() => undefined);
  }, []);

  const runSearch = useCallback(() => {
    const q = [query, FILTER_QUERY[filter]].filter(Boolean).join(" ");
    apiSearch(q, sort === "relevance" ? undefined : sort, 200)
      .then((r) => {
        setItems(r.items);
        setTotal(r.total);
        setElapsed(r.elapsedMs);
      })
      .catch(() => undefined);
  }, [query, filter, sort]);

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
      if (event.key === "ArrowDown" && items.length > 0) {
        event.preventDefault();
        setSelectedIndex((i) => Math.min(i + 1, items.length - 1));
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        setSelectedIndex((i) => Math.max(i - 1, 0));
      } else if (event.key === " ") {
        event.preventDefault();
        const it = items[selectedIndex];
        if (it) setPreviewPath(it.path);
      } else if (event.key === "Escape") {
        event.preventDefault();
        if (query) setQuery("");
        else void getCurrentWindow().hide();
      } else if (event.key === "l" && (event.ctrlKey || event.metaKey)) {
        event.preventDefault();
        inputRef.current?.focus();
        inputRef.current?.select();
      }
    },
    [items, selectedIndex, query, previewPath, menuFor]
  );

  return (
    <div className="app" onKeyDown={onKeyDown}>
      <TitleBar />
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
        <div className="sort-wrap">
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
      />
      <FooterBar />
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
