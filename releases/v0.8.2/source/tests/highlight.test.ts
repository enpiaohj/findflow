import assert from "node:assert/strict";
import test from "node:test";

import { queryTerms, splitHighlight } from "../src/highlight.ts";

test("queryTerms 忽略过滤器、排除词，去掉通配符并按长度降序（等长保持原序）", () => {
  assert.deepEqual(queryTerms("报告 ext:pdf -草稿 *年度*"), ["报告", "年度"]);
  assert.deepEqual(queryTerms("  "), []);
  assert.deepEqual(queryTerms('报告 path:"D:\My Docs" -path:"E:\a b"'), ["报告"]);
  assert.deepEqual(queryTerms("ab abcd"), ["abcd", "ab"]);
  assert.deepEqual(queryTerms("Report report"), ["report"]);
});

test("splitHighlight 不区分大小写并合并相邻命中", () => {
  assert.deepEqual(splitHighlight("Annual Report.pdf", ["report"]), [
    { text: "Annual ", hit: false },
    { text: "Report", hit: true },
    { text: ".pdf", hit: false },
  ]);
  assert.deepEqual(splitHighlight("abab", ["ab"]), [{ text: "abab", hit: true }]);
});

test("splitHighlight 无关键词或无命中时返回整段", () => {
  assert.deepEqual(splitHighlight("文件.txt", []), [{ text: "文件.txt", hit: false }]);
  assert.deepEqual(splitHighlight("文件.txt", ["xyz"]), [{ text: "文件.txt", hit: false }]);
});
