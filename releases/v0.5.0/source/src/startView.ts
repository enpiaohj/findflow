import type { ScopeInfo } from "./api";

export const START_VIEW_KEY = "ui.startView";
export const LEGACY_START_LOCATION_KEY = "ui.startLocation";

export type StartViewMode = "index" | "large";
export type StartViewSort = "relevance" | "name" | "size";

export interface StartViewSetting {
  mode: StartViewMode;
  scopeId: number | null;
}

export interface ResolvedStartView {
  setting: StartViewSetting;
  needsPersist: boolean;
}

export interface StartViewQueryDefaults {
  sort: StartViewSort;
  scopeId?: number;
}

export const DEFAULT_START_VIEW: Readonly<StartViewSetting> = Object.freeze({
  mode: "index",
  scopeId: null,
} as const);

export function resolveStartView(
  currentValue: unknown,
  legacyValue: unknown,
  scopes: ScopeInfo[],
): ResolvedStartView {
  if (currentValue === null || currentValue === undefined) {
    const legacyScopeId = isScopeId(legacyValue) ? legacyValue : null;
    return normalizeStartView(
      { mode: "index", scopeId: legacyScopeId },
      scopes,
      true,
    );
  }

  const setting = parseStartView(currentValue);
  if (setting === null) {
    return normalizeStartView({ ...DEFAULT_START_VIEW }, scopes, true);
  }

  return normalizeStartView(setting, scopes, false);
}

export function getStartViewQueryDefaults(
  setting: StartViewSetting,
  hasQuery: boolean,
): StartViewQueryDefaults {
  if (hasQuery) {
    return { sort: "relevance" };
  }

  if (setting.mode === "large") {
    return { sort: "size" };
  }

  return setting.scopeId === null
    ? { sort: "name" }
    : { sort: "name", scopeId: setting.scopeId };
}

function parseStartView(value: unknown): StartViewSetting | null {
  if (!isRecord(value) || (value.mode !== "index" && value.mode !== "large")) {
    return null;
  }

  if (value.scopeId !== null && !isScopeId(value.scopeId)) {
    return null;
  }

  return { mode: value.mode, scopeId: value.scopeId };
}

function normalizeStartView(
  setting: StartViewSetting,
  scopes: ScopeInfo[],
  needsPersist: boolean,
): ResolvedStartView {
  if (setting.mode === "large") {
    return {
      setting: { mode: "large", scopeId: null },
      needsPersist: needsPersist || setting.scopeId !== null,
    };
  }

  const enabledScopeId = scopes.find(
    (scope) => scope.enabled && isScopeId(scope.scopeId),
  )?.scopeId ?? null;
  const selectedScopeEnabled = setting.scopeId !== null && scopes.some(
    (scope) => scope.scopeId === setting.scopeId && scope.enabled,
  );

  if (selectedScopeEnabled || (setting.scopeId === null && enabledScopeId === null)) {
    return { setting, needsPersist };
  }

  return {
    setting: { mode: "index", scopeId: enabledScopeId },
    needsPersist: true,
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isScopeId(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value > 0;
}
