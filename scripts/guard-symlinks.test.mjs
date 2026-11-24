import assert from "node:assert/strict";
import { test } from "vitest";

import { checkSymlinks, indexLinks } from "./guard/symlinks.mjs";

// The tracked-link guard (scripts/guard/symlinks.mjs, ADR 1210), on in-memory indexes.
const project = "fixtures/reactive-ir/linked";
const tracked = [`${project}/packages/kernel/index.js`, `${project}/App.tsx`, "packages/cli/package.json"];
const problemOf = (path, target) => checkSymlinks([{ path, target }], tracked).map((entry) => entry.problem);

test("a fixture's relative workspace link to a tracked directory or file passes", () => {
  assert.deepEqual(problemOf(`${project}/node_modules/@ws/kernel`, "../../packages/kernel"), []);
  assert.deepEqual(problemOf(`${project}/node_modules/app`, "../App.tsx"), []);
});

test("an absolute target fails wherever it is (ADR 1165's node_modules incident)", () => {
  assert.match(problemOf("packages/cli/node_modules", "/Users/someone/dev/other-worktree/packages/cli/node_modules")[0], /absolute/);
  assert.match(problemOf(`${project}/node_modules/@ws/kernel`, "/tmp/kernel")[0], /absolute/);
  assert.match(problemOf(`${project}/node_modules/x`, "~/x")[0], /absolute/);
  assert.match(problemOf(`${project}/node_modules/x`, "C:\\x")[0], /absolute/);
});

test("a relative link outside a fixture project fails", () => {
  assert.match(problemOf("packages/cli/node_modules", "../../node_modules")[0], /outside a fixture project/);
  assert.match(problemOf("fixtures/README-link", "reactive-ir")[0], /outside a fixture project/);
});

test("a target that leaves its fixture project, or that git does not track, fails", () => {
  assert.match(problemOf(`${project}/node_modules/x`, "../../other/packages/x")[0], /outside its fixture project/);
  assert.match(problemOf(`${project}/node_modules/x`, "../../../../packages/cli")[0], /outside its fixture project/);
  assert.match(problemOf(`${project}/node_modules/@ws/gone`, "../../packages/gone")[0], /does not track/);
});

test("the repository's own index passes", () => {
  const { links, tracked: paths } = indexLinks();
  assert.deepEqual(checkSymlinks(links, paths), []);
});
