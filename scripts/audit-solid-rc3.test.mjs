import assert from "node:assert/strict";
import { test } from "vitest";

import {
  assertSafeArchiveEntries,
  canonicalize,
  collectExportTargets
} from "./audit-solid-rc3.mjs";

test("canonicalize sorts object keys recursively without sorting arrays", () => {
  assert.deepEqual(canonicalize({ z: { b: 2, a: 1 }, a: [2, 1] }), {
    a: [2, 1],
    z: { a: 1, b: 2 }
  });
});

test("collectExportTargets preserves ordered condition traces and target kinds", () => {
  assert.deepEqual(
    collectExportTargets({
      ".": {
        browser: { import: { types: "./types/index.d.ts", default: "./dist/web.js" } }
      },
      "./types/*": "./types/*"
    }),
    [
      {
        trace: [".", "browser", "import", "types"],
        target: "./types/index.d.ts",
        kind: "declaration",
        pattern: false
      },
      {
        trace: [".", "browser", "import", "default"],
        target: "./dist/web.js",
        kind: "runtime",
        pattern: false
      },
      {
        trace: ["./types/*"],
        target: "./types/*",
        kind: "other",
        pattern: true
      }
    ]
  );
});

test("archive entries must stay under package and cannot traverse", () => {
  assert.doesNotThrow(() => assertSafeArchiveEntries(["package/package.json", "package/dist/index.js"]));
  assert.throws(() => assertSafeArchiveEntries(["../escape"]), /unsafe tar entry/);
  assert.throws(() => assertSafeArchiveEntries(["package/../escape"]), /unsafe tar entry/);
});
