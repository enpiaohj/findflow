import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import TitleBar from "./components/TitleBar";
import FilterBar, { type FilterId } from "./components/FilterBar";
import ResultList from "./components/ResultList";
import FooterBar from "./components/FooterBar";
import { Toast } from "./components/Toast";
import { SearchIcon } from "./components/Icons";
import { MOCK_FILES } from "./data/mockFiles";
import type { Density } from "./types";

const DENSITY_KEY = "ui.density";

export default function App() {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<FilterId>("all");
  const [density, setDensity] = useState<Density>("compact");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [toast, setToast] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const toastTimer = useRef<number | null>(null);

  useEffect(() => {
    invoke("get_setting", { key: DENSITY_KEY })
      .then((value) => {
        if (typeof value === "string") setDensity(value as Density);
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
    invoke("set_setting", { key: DENSITY_KEY, value: next }).catch(() => undefined);
  }, []);

  const items = useMemo(() => {
    let list = MOCK_FILES;
    const q = query.trim().toLowerCase();
    if (q) {
      list = list.filter(
        (item) => item.name.toLowerCase().includes(q) || item.path.toLowerCase().includes(q)
      );
    }
    if (filter === "document") {
      list = list.filter((item) => ["doc", "sheet", "pdf"].includes(item.kind));
    } else if (filter === "folder") {
      list = list.filter((item) => item.kind === "folder");
    } else if (filter === "image") {
      list = list.filter((item) => item.kind === "image" || item.kind === "video");
    } else if (filter === "recent") {
      list = list.slice(0, 8);
    }
    return list;
  }, [query, filter]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query, filter, density]);

  const onKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>) => {
      if (event.key === "ArrowDown" && items.length > 0) {
        event.preventDefault();
        setSelectedIndex((index) => Math.min(index + 1, items.length - 1));
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        setSelectedIndex((index) => Math.max(index - 1, 0));
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
      }
    },
    [items.length, query]
  );

  return (
    <div className="app" onKeyDown={onKeyDown}>
      <TitleBar onNotice={notice} />
      <div className="searchbar">
        <span className="searchbar-icon">
          <SearchIcon />
        </span>
        <input
          ref={inputRef}
          className="searchbar-input"
          placeholder="搜索文件名、路径或类型，如 ext:docx"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          autoFocus
        />
        <span className="searchbar-kbd">Alt + Space</span>
      </div>
      <FilterBar
        filter={filter}
        onFilterChange={setFilter}
        density={density}
        onDensityChange={changeDensity}
        count={items.length}
      />
      <ResultList
        items={items}
        density={density}
        selectedIndex={selectedIndex}
        onSelect={setSelectedIndex}
        onNotice={notice}
      />
      <FooterBar />
      {toast ? <Toast message={toast} /> : null}
    </div>
  );
}
