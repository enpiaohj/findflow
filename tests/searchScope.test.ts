import assert from "node:assert/strict";
import test from "node:test";

import { folderFilterOf, withPathFilter } from "../src/searchScope.ts";

test("folderFilterOf 取所在文件夹，含空格时加引号", () => {
  assert.equal(folderFilterOf("D:\\Temp\\a.txt"), "path:D:\\Temp");
  assert.equal(folderFilterOf("D:\\My Docs\\a.txt"), 'path:"D:\\My Docs"');
  assert.equal(folderFilterOf("D:\\a.txt"), "path:D:\\");
  assert.equal(folderFilterOf("a.txt"), null);
});

test("withPathFilter 替换已有 path 条件但保留排除条件", () => {
  assert.equal(withPathFilter("报告", "path:D:\\Temp"), "报告 path:D:\\Temp");
  assert.equal(withPathFilter("报告 path:E:\\x", "path:D:\\Temp"), "报告 path:D:\\Temp");
  assert.equal(withPathFilter('报告 path:"E:\\a b"', "path:D:\\Temp"), "报告 path:D:\\Temp");
  assert.equal(withPathFilter("报告 -path:old", "path:D:\\Temp"), "报告 -path:old path:D:\\Temp");
  assert.equal(withPathFilter("", "path:D:\\Temp"), "path:D:\\Temp");
});
