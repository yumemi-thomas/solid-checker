import assert from "node:assert/strict";
import { test } from "vitest";

import { BUDGETS, corpusManifest, measure, trackedFiles, violations } from "./repro-budget.mjs";

const commit = "35c4cc6b7c932f5dee5815b37d3601e7dff90b58";
const sha = "0".repeat(64);
const valid = () => ({
  projects: ["sc2001"],
  sources: { "sc2001/src/a.tsx": 10 },
  modules: { "node_modules/solid-js/package.json": { size: 100, sha256: sha } },
  table: {
    cases: [
      {
        id: "a",
        kind: "uncertifiable",
        project: "sc2001",
        files: ["src/a.tsx"],
        causes: ["SC2001/unplaced-write/after-await"],
        expect: [{ rule: "SC2001", family: "unplaced-write", context: "", cause: "SC2001/unplaced-write/after-await" }],
        source: { app: "one", path: "src/a.tsx", line: 3, commit },
        weight: 20,
        status: "uncertifiable",
      },
    ],
  },
  packages: { packages: [{ name: "solid-js", version: "2.0.0-rc.17", integrity: "sha512-x", files: { "package.json": sha } }] },
});

test("the reproduction set is within its budget", () => {
  assert.deepEqual(violations(measure(), { tracked: trackedFiles(), manifest: corpusManifest() }), []);
});

test("a valid synthetic set has no violation", () => {
  assert.deepEqual(violations(valid(), { manifest: { apps: { one: { commit } } } }), []);
});

test("a long file, too many projects and a large node_modules are each reported", () => {
  const state = valid();
  state.sources["sc2001/src/a.tsx"] = BUDGETS.fileLines + 1;
  state.projects = Array.from({ length: BUDGETS.projects + 1 }, (_, index) => `p${index}`).concat("sc2001");
  state.modules["node_modules/solid-js/package.json"].size = BUDGETS.nodeModulesBytes + 1;
  const found = violations(state);
  assert.ok(found.some((line) => /lines, budget \d+ per file/.test(line)));
  assert.ok(found.some((line) => /projects, budget/.test(line)));
  assert.ok(found.some((line) => /node_modules: .* bytes, budget/.test(line)));
});

test("a source file no case owns, and a case with three files, are reported", () => {
  const state = valid();
  state.sources["sc2001/src/b.tsx"] = 5;
  state.table.cases[0].files = ["src/a.tsx", "src/b.tsx", "src/c.tsx"];
  const found = violations(state);
  assert.ok(found.some((line) => /3 files, budget/.test(line)));
  assert.ok(found.some((line) => /src\/c\.tsx is not a source file/.test(line)));
});

test("an unlisted package file, a changed byte and a missing listed file are reported", () => {
  const state = valid();
  state.modules["node_modules/left-pad/index.js"] = { size: 1, sha256: sha };
  state.modules["node_modules/solid-js/package.json"].sha256 = "1".repeat(64);
  state.packages.packages[0].files["dist/solid.js"] = sha;
  const found = violations(state);
  assert.ok(found.some((line) => /left-pad\/index\.js: not listed/.test(line)));
  assert.ok(found.some((line) => /solid-js\/package\.json: sha256/.test(line)));
  assert.ok(found.some((line) => /dist\/solid\.js: listed in packages\.json but absent/.test(line)));
});

test("a source pinned at another commit than the corpus manifest, and an untracked file, are reported", () => {
  const found = violations(valid(), { tracked: new Set(), manifest: { apps: { one: { commit: "f".repeat(40) } } } });
  assert.ok(found.some((line) => /is pinned at f{40}/.test(line)));
  assert.ok(found.some((line) => /not tracked by git/.test(line)));
});

test("each kind of case keeps its own source budget and its own statuses", () => {
  const state = valid();
  const lines = BUDGETS.kinds.violation.lines;
  state.sources["sc2001/src/v.tsx"] = lines;
  state.sources["sc2001/src/w.tsx"] = 1;
  state.table.cases.push({
    id: "v",
    kind: "violation",
    project: "sc2001",
    files: ["src/v.tsx", "src/w.tsx"],
    expect: [{ rule: "SC2001", family: "unplaced-write", context: "" }],
    source: { app: "one", path: "src/v.tsx", line: 1, commit },
    weight: 1,
    status: "uncertifiable",
  });
  const found = violations(state);
  assert.ok(found.some((line) => /violation cases: \d+ lines, budget/.test(line)));
  assert.ok(found.some((line) => /status uncertifiable is not one a violation case has/.test(line)));
  assert.ok(!found.some((line) => /uncertifiable cases: .* budget/.test(line)));
});

test("a clean case expects no result, and an unknown kind is reported", () => {
  const state = valid();
  state.table.cases[0].kind = "clean";
  state.table.cases[0].status = "certified";
  assert.ok(violations(state).some((line) => /a clean case expects no result/.test(line)));
  state.table.cases[0].kind = "flaky";
  assert.ok(violations(state).some((line) => /kind flaky is not one of/.test(line)));
});
