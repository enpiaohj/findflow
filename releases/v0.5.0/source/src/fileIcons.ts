import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/**
 * 系统文件图标 / 缩略图加载：
 * - 图标按扩展名共享（exe / lnk 等自带图标的类型按路径），键规则与后端 shellicon::icon_key 一致；
 * - 同一帧内的请求合并为一次 IPC，已取到（或确认取不到）的结果全局缓存；
 * - 取不到时返回 null，由调用方显示内置图标。
 */

const PER_PATH_EXTS = new Set(["exe", "lnk", "ico", "url", "msc", "cpl", "scr", "appref-ms"]);

export interface IconSubject {
  path: string;
  ext: string;
  isDir: boolean;
}

/** 按设备像素比取整到常用位图尺寸，高分屏下图标保持清晰。 */
export function iconPixelSize(cssSize: number): number {
  const want = Math.ceil(cssSize * (window.devicePixelRatio || 1));
  return [16, 24, 32, 48, 64, 96, 128, 256].find((s) => s >= want) ?? 256;
}

export function iconKey(s: IconSubject, size: number): string {
  const ext = s.ext.toLowerCase();
  if (s.isDir) return `dir@${size}`;
  if (PER_PATH_EXTS.has(ext)) return `path:${s.path.toLowerCase()}@${size}`;
  return `ext:${ext}@${size}`;
}

const iconCache = new Map<string, string | null>();
const listeners = new Map<string, Set<(v: string | null) => void>>();
const queued = new Map<number, Map<string, IconSubject>>();
let flushScheduled = false;

function flush() {
  flushScheduled = false;
  const batches = Array.from(queued.entries());
  queued.clear();
  for (const [size, subjects] of batches) {
    const items = Array.from(subjects.values()).map((s) => ({ path: s.path, ext: s.ext, isDir: s.isDir }));
    invoke<Record<string, string | null>>("file_icons", { items, size })
      .catch(() => ({}) as Record<string, string | null>)
      .then((result) => {
        for (const key of subjects.keys()) {
          const value = result[key] ?? null;
          iconCache.set(key, value);
          listeners.get(key)?.forEach((cb) => cb(value));
          listeners.delete(key);
        }
      });
  }
}

function requestIcon(s: IconSubject, size: number, cb: (v: string | null) => void): () => void {
  const key = iconKey(s, size);
  let set = listeners.get(key);
  const isNew = !set;
  if (!set) {
    set = new Set();
    listeners.set(key, set);
  }
  set.add(cb);
  if (isNew) {
    let batch = queued.get(size);
    if (!batch) {
      batch = new Map();
      queued.set(size, batch);
    }
    batch.set(key, s);
    if (!flushScheduled) {
      flushScheduled = true;
      requestAnimationFrame(flush);
    }
  }
  return () => {
    listeners.get(key)?.delete(cb);
  };
}

/** 文件系统图标：返回 data URL；加载中或取不到时为 null。 */
export function useFileIcon(s: IconSubject, cssSize: number): string | null {
  const size = iconPixelSize(cssSize);
  const key = iconKey(s, size);
  const [value, setValue] = useState<string | null>(() => iconCache.get(key) ?? null);
  useEffect(() => {
    const cached = iconCache.get(key);
    if (cached !== undefined) {
      setValue(cached);
      return;
    }
    setValue(null);
    return requestIcon(s, size, setValue);
    // key 已覆盖 path / ext / isDir / size 的变化
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  return value;
}

const thumbCache = new Map<string, string | null>();
const THUMB_CACHE_MAX = 300;

/** 系统缩略图（图片 / 视频 / 文档）：返回 data URL；无缩略图时为 null。 */
export function useThumbnail(
  path: string,
  modifiedMs: number,
  cssSize: number,
  enabled: boolean
): string | null {
  const size = iconPixelSize(cssSize);
  const key = `${path.toLowerCase()}|${modifiedMs}|${size}`;
  const [value, setValue] = useState<string | null>(() => thumbCache.get(key) ?? null);
  useEffect(() => {
    if (!enabled) {
      setValue(null);
      return;
    }
    const cached = thumbCache.get(key);
    if (cached !== undefined) {
      setValue(cached);
      return;
    }
    setValue(null);
    let alive = true;
    invoke<string | null>("file_thumbnail", { path, modifiedMs, size })
      .catch(() => null)
      .then((v) => {
        if (thumbCache.size >= THUMB_CACHE_MAX) thumbCache.clear();
        thumbCache.set(key, v);
        if (alive) setValue(v);
      });
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, enabled]);
  return value;
}
