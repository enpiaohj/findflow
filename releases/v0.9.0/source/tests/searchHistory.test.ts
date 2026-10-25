import assert from "node:assert/strict";
import test from "node:test";

import {
  parseSearchHistory,
  recordSearch,
  removeSearch,
} from "../src/searchHistory.ts";

test("解析非法历史值一律回退为空数组", () => {
  assert.deepEqual(parseSearchHistory(null), []);
  assert.deepEqual(parseSearchHistory("broken"), []);
  assert.deepEqual(parseSearchHistory([1, "a", "  ", "b"]), ["a", "b"]);
});

test("记录搜索词置顶并去重，容量上限丢弃最旧", () => {
  assert.deepEqual(recordSearch([], "readme"), ["readme"]);
  assert.deepEqual(recordSearch(["readme"], "报表"), ["报表", "readme"]);
  // 与最近一条相同：不重复记录，也不改变顺序
  assert.deepEqual(recordSearch(["readme", "报表"], "readme"), ["readme", "报表"]);
  // 同名旧条目移除后置顶
  assert.deepEqual(recordSearch(["报表", "readme"], "报表"), ["报表", "readme"]);
  // 空白搜索词不记录
  assert.deepEqual(recordSearch(["readme"], "   "), ["readme"]);

  const full = Array.from({ length: 50 }, (_, i) => `词${i}`);
  const next = recordSearch(full, "新词");
  assert.equal(next.length, 50);
  assert.equal(next[0], "新词");
  assert.equal(next.includes("词49"), false);
});

test("删除指定历史项", () => {
  assert.deepEqual(removeSearch(["a", "b", "c"], "b"), ["a", "c"]);
  assert.deepEqual(removeSearch(["a", "b"], "不存在"), ["a", "b"]);
});
