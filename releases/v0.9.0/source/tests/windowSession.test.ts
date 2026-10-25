import { test } from "node:test";
import { strict as assert } from "node:assert";
import { shouldClearSession } from "../src/windowSession";

test("只有实际主动隐藏和自动隐藏结束搜索会话", () => {
  assert.equal(shouldClearSession("manual"), true);
  assert.equal(shouldClearSession("blur"), true);
  assert.equal(shouldClearSession("open-file"), false);
  assert.equal(shouldClearSession("focus-lost"), false);
});
