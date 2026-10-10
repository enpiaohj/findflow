import assert from "node:assert/strict";
import test from "node:test";

import { progressRatio, progressText } from "../src/updateProgress.ts";

const MB = 1024 * 1024;

test("progressRatio 在总大小未知或为 0 时返回 null，并限制在 0–1", () => {
  assert.equal(progressRatio({ downloaded: 5, total: null, finished: false }), null);
  assert.equal(progressRatio({ downloaded: 5, total: 0, finished: false }), null);
  assert.equal(progressRatio({ downloaded: 5 * MB, total: 10 * MB, finished: false }), 0.5);
  // 服务器给的总大小偏小时不超过 100%
  assert.equal(progressRatio({ downloaded: 12 * MB, total: 10 * MB, finished: false }), 1);
});

test("progressText 按已知 / 未知总大小与完成状态给出文案", () => {
  assert.equal(
    progressText({ downloaded: 3.2 * MB, total: 13.5 * MB, finished: false }),
    "正在下载 3.2 / 13.5 MB（24%）",
  );
  assert.equal(progressText({ downloaded: 1.5 * MB, total: null, finished: false }), "正在下载 1.5 MB…");
  assert.equal(
    progressText({ downloaded: 13.5 * MB, total: 13.5 * MB, finished: true }),
    "下载完成，正在启动安装程序…",
  );
});
