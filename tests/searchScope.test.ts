import assert from "node:assert/strict";
import test from "node:test";

import { folderFilterOf, pathQueryOf, withPathFilter } from "../src/searchScope.ts";

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

test("pathQueryOf 只把整体是绝对路径的搜索框内容当路径", () => {
  assert.equal(pathQueryOf("D:\\Temp"), "D:\\Temp");
  assert.equal(pathQueryOf("  d:/temp/  "), "d:/temp/");
  assert.equal(pathQueryOf('"D:\\My Docs"'), "D:\\My Docs");
  assert.equal(pathQueryOf("\\\\srv\\share\\x"), "\\\\srv\\share\\x");
  assert.equal(pathQueryOf("d:"), null);
  assert.equal(pathQueryOf("ext:docx"), null);
  assert.equal(pathQueryOf("报告 D:\\Temp"), null);
  assert.equal(pathQueryOf(""), null);
});
