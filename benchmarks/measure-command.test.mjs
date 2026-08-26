import assert from "node:assert/strict";
import { test } from "vitest";

import { descendantRssKiB, parseProcessTable } from "./measure-command.mjs";

test("parseProcessTable ignores malformed rows", () => {
  assert.deepEqual(parseProcessTable(" 10 1 200\nnoise\n11 10 300\n"), [
    { pid: 10, ppid: 1, rssKiB: 200 },
    { pid: 11, ppid: 10, rssKiB: 300 }
  ]);
});

test("descendantRssKiB includes the complete process tree and no siblings", () => {
  const rows = [
    { pid: 10, ppid: 1, rssKiB: 100 },
    { pid: 11, ppid: 10, rssKiB: 200 },
    { pid: 12, ppid: 11, rssKiB: 300 },
    { pid: 13, ppid: 1, rssKiB: 400 }
  ];
  assert.equal(descendantRssKiB(10, rows), 600);
});
