import { getSetting, setSetting } from "./api";

/** 主题偏好：跟随系统 / 浅色 / 深色。 */
export type ThemePref = "system" | "light" | "dark";

const KEY = "ui.theme";
const media = window.matchMedia("(prefers-color-scheme: dark)");
let current: ThemePref = "system";

function resolve(pref: ThemePref): "light" | "dark" {
  if (pref === "system") return media.matches ? "dark" : "light";
  return pref;
}

function apply() {
  document.documentElement.dataset.theme = resolve(current);
}

// 跟随系统时，系统切换深浅色即时生效
media.addEventListener("change", () => {
  if (current === "system") apply();
});

/** 启动时读取偏好并应用（读取前先按系统主题渲染，避免闪白）。 */
export function initTheme() {
  apply();
  getSetting<ThemePref>(KEY)
    .then((v) => {
      if (v === "light" || v === "dark" || v === "system") {
        current = v;
        apply();
      }
    })
    .catch(() => undefined);
}

export function getThemePref(): ThemePref {
  return current;
}

export function setThemePref(pref: ThemePref) {
  current = pref;
  apply();
  setSetting(KEY, pref).catch(() => undefined);
}
