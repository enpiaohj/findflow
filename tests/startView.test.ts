import assert from "node:assert/strict";
import test from "node:test";

import type { ScopeInfo } from "../src/api.ts";
import {
  DEFAULT_START_VIEW,
  getStartViewQueryDefaults,
  resolveStartView,
} from "../src/startView.ts";

const enabledScopes: ScopeInfo[] = [
  {
    scopeId: 1,
    path: "C:\\资料",
    enabled: true,
    state: "ready",
    filesCount: 120,
    baselineAt: "2026-09-27T00:00:00Z",
    optionsJson: "{}",
  },
  {
    scopeId: 2,
    path: "D:\\归档",
    enabled: true,
    state: "ready",
    filesCount: 80,
    baselineAt: "2026-09-27T00:00:00Z",
    optionsJson: "{}",
  },
];

test("缺少新旧设置时使用第一个启用位置的索引视图", () => {
  assert.deepEqual(resolveStartView(undefined, undefined, enabledScopes), {
    setting: { mode: "index", scopeId: 1 },
    needsPersist: true,
  });
});

test("新设置缺失时迁移有效的旧位置编号", () => {
  assert.deepEqual(resolveStartView(null, 2, enabledScopes), {
    setting: { mode: "index", scopeId: 2 },
    needsPersist: true,
  });
});

test('旧位置值为 "all" 时迁移为第一个启用位置的索引视图', () => {
  assert.deepEqual(resolveStartView(undefined, "all", enabledScopes), {
    setting: { mode: "index", scopeId: 1 },
    needsPersist: true,
  });
});

test("新设置非法时忽略旧位置并回退默认位置", () => {
  assert.deepEqual(resolveStartView("broken", 2, enabledScopes), {
    setting: { mode: "index", scopeId: 1 },
    needsPersist: true,
  });
});

test("已选位置停用时回退到第一个启用位置", () => {
  const scopes = [
    { ...enabledScopes[0], enabled: false },
    ...enabledScopes.slice(1),
  ];

  assert.deepEqual(resolveStartView({ mode: "index", scopeId: 1 }, null, scopes), {
    setting: { mode: "index", scopeId: 2 },
    needsPersist: true,
  });
});

test("没有启用位置时将索引视图保留为无位置", () => {
  const scopes = enabledScopes.map((scope) => ({ ...scope, enabled: false }));

  assert.deepEqual(resolveStartView({ mode: "index", scopeId: 2 }, null, scopes), {
    setting: { mode: "index", scopeId: null },
    needsPersist: true,
  });
});

test("large 设置会清除携带的位置编号", () => {
  assert.deepEqual(resolveStartView({ mode: "large", scopeId: 2 }, null, enabledScopes), {
    setting: { mode: "large", scopeId: null },
    needsPersist: true,
  });
});

test("根据启动视图和关键词生成查询默认值", () => {
  assert.deepEqual(
    getStartViewQueryDefaults({ mode: "index", scopeId: 2 }, false),
    { sort: "name", scopeId: 2 },
  );
  assert.deepEqual(
    getStartViewQueryDefaults({ mode: "large", scopeId: null }, false),
    { sort: "size" },
  );
  assert.deepEqual(
    getStartViewQueryDefaults({ mode: "index", scopeId: 2 }, true),
    { sort: "relevance" },
  );
});

test("合法新设置优先于冲突的旧位置", () => {
  assert.deepEqual(resolveStartView({ mode: "index", scopeId: 2 }, 1, enabledScopes), {
    setting: { mode: "index", scopeId: 2 },
    needsPersist: false,
  });
});
