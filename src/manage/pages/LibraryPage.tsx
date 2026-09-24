import { useEffect, useState } from "react";
import type { PageCtx } from "../ManageApp";
import {
  favoritesList,
  favoriteToggle,
  tagsList,
  tagCreate,
  tagDelete,
  recentsList,
  actionOpen,
  actionLocate,
  formatTime,
  type FileRow,
} from "../../api";

export default function LibraryPage({ notice }: PageCtx) {
  const [tab, setTab] = useState<"favorites" | "tags" | "recents">("favorites");
  const [favs, setFavs] = useState<FileRow[]>([]);
  const [tags, setTags] = useState<{ tagId: number; name: string; color: string }[]>([]);
  const [recents, setRecents] = useState<{ path: string; action: string; at: string }[]>([]);
  const [newTag, setNewTag] = useState("");

  const reload = () => {
    favoritesList().then(setFavs).catch(() => undefined);
    tagsList().then(setTags).catch(() => undefined);
    recentsList().then(setRecents).catch(() => undefined);
  };
  useEffect(reload, []);

  return (
    <div className="page">
      <h2>收藏与标签</h2>
      <div className="tab-row">
        <button className={tab === "favorites" ? "tab-active" : ""} onClick={() => setTab("favorites")}>
          收藏（{favs.length}）
        </button>
        <button className={tab === "tags" ? "tab-active" : ""} onClick={() => setTab("tags")}>
          标签（{tags.length}）
        </button>
        <button className={tab === "recents" ? "tab-active" : ""} onClick={() => setTab("recents")}>
          最近使用（保留 90 天）
        </button>
      </div>

      {tab === "favorites" ? (
        <section className="card">
          {favs.length === 0 ? <p className="muted">还没有收藏。在搜索结果的「更多」菜单中收藏文件。</p> : null}
          {favs.map((f) => (
            <div key={f.fileId} className="dup-file">
              <span className="dup-name">{f.name}</span>
              <span className="muted cell-path">{f.path}</span>
              <button onClick={() => actionOpen(f.path).catch((e) => notice(String(e)))}>打开</button>
              <button onClick={() => actionLocate(f.path).catch((e) => notice(String(e)))}>定位</button>
              <button
                className="btn-danger-text"
                onClick={() =>
                  favoriteToggle(f.fileId)
                    .then(() => reload())
                    .catch((e) => notice(String(e)))
                }
              >
                取消收藏
              </button>
            </div>
          ))}
        </section>
      ) : null}

      {tab === "tags" ? (
        <section className="card">
          <div className="card-head">
            <h3>标签定义</h3>
            <div className="tag-create">
              <input
                value={newTag}
                placeholder="标签名"
                onChange={(e) => setNewTag(e.target.value)}
              />
              <button
                onClick={() => {
                  if (!newTag.trim()) return;
                  tagCreate(newTag.trim(), "#2563eb")
                    .then(() => {
                      setNewTag("");
                      reload();
                    })
                    .catch((e) => notice(String(e)));
                }}
              >
                新建
              </button>
            </div>
          </div>
          {tags.map((t) => (
            <div key={t.tagId} className="dup-file">
              <span className="tag-dot" style={{ background: t.color }} />
              <span className="dup-name">{t.name}</span>
              <button
                className="btn-danger-text"
                onClick={() =>
                  tagDelete(t.tagId)
                    .then(reload)
                    .catch((e) => notice(String(e)))
                }
              >
                删除
              </button>
            </div>
          ))}
          <p className="muted">在搜索结果的「更多」菜单中为文件打标签。</p>
        </section>
      ) : null}

      {tab === "recents" ? (
        <section className="card">
          {recents.map((r, i) => (
            <div key={i} className="dup-file">
              <span className="dup-name">{r.path.split("\\").pop()}</span>
              <span className="muted cell-path">{r.path}</span>
              <span className="muted">
                {r.action} · {formatTime(Date.parse(r.at))}
              </span>
            </div>
          ))}
          {recents.length === 0 ? <p className="muted">暂无记录。最近使用只记录由 FindFlow 发起的打开行为。</p> : null}
        </section>
      ) : null}
    </div>
  );
}
