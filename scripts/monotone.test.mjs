import assert from "node:assert/strict";
import { test } from "vitest";

import { identity, keySpan, moves, namingLines, siteCodes, unnamedUp } from "./monotone.mjs";

// ADR 1118: a proof change moves a (site, code) only up, from U to clean or to V.
const root = "/p";
const finding = (id, kind, start, path = "/p/App.tsx") => ({ id, kind, primaryLocation: { path, startByte: start, endByte: start + 3 } });

test("siteCodes: one entry per (file, span, code); a violation of the code there makes it V; the fuzz entry is left out", () => {
  const sites = siteCodes({ findings: [finding("SC1001", "uncertifiable", 10), finding("SC1001", "violation", 10), finding("SC2001", "uncertifiable", 10)] }, root);
  assert.deepEqual([...sites], [["App.tsx@10-13|SC1001", "V"], ["App.tsx@10-13|SC2001", "U"]]);
  const fuzz = "/i/.solid-checker-fuzz/run/x";
  assert.deepEqual([...siteCodes({ findings: [finding("SC1002", "uncertifiable", 5, `${fuzz}/main.tsx`)] }, fuzz)], []);
  // a fixture's own main.tsx is the program's
  assert.equal(siteCodes({ findings: [finding("SC1002", "uncertifiable", 5, "/p/main.tsx")] }, root).size, 1);
});

test("moves: U to clean and U to V are up; every other move is bad, whichever way", () => {
  const base = new Map([["a|SC1", "U"], ["b|SC1", "U"], ["c|SC1", "V"], ["d|SC1", "V"], ["f|SC1", "U"]]);
  const head = new Map([["b|SC1", "V"], ["c|SC1", "U"], ["e|SC1", "U"], ["f|SC1", "U"], ["g|SC1", "V"]]);
  const { up, bad } = moves(base, head);
  assert.deepEqual(up, [{ site: "a|SC1", from: "U", to: "clean" }, { site: "b|SC1", from: "U", to: "V" }]);
  assert.deepEqual(bad, [{ site: "c|SC1", from: "V", to: "U" }, { site: "d|SC1", from: "V", to: "clean" }, { site: "e|SC1", from: "clean", to: "U" }, { site: "g|SC1", from: "clean", to: "V" }]);
});

test("identity: every field of every finding, in the report's order", () => {
  const one = { findings: [finding("SC1", "uncertifiable", 1), finding("SC2", "uncertifiable", 2)] };
  assert.equal(identity(one), identity(structuredClone(one)));
  assert.notEqual(identity(one), identity({ findings: [...one.findings].reverse() }));
  assert.notEqual(identity(one), identity({ findings: [{ ...one.findings[0], message: "other" }, one.findings[1]] }));
});

test("M2b: an up move must be named, in its file on an overlapping span, by a decision of one of the change's arms or a silence it names", () => {
  assert.deepEqual(keySpan("App.tsx@10-13|SC1001"), { path: "App.tsx", start: 10, end: 13 });
  assert.equal(keySpan("App.tsx@1:2-1:4|SC1001"), null);
  const log = [
    { k: "path", f: "/p/App.tsx", p: 1, q: 9 },
    { k: "decision", p: 1, q: 9, s: 8, e: 20, arm: "callee_roles.new-arm", arms: ["x.y", "callee_roles.new-arm"] },
    { k: "silent", p: 1, q: 9, s: 40, e: 44, proof: "after_await_accounting.account.new-reason" },
    { k: "site", p: 1, q: 9, s: 60, e: 64 },
  ].map((line) => JSON.stringify(line)).join("\n");
  const lines = namingLines(log, "/p");
  assert.deepEqual(lines.map((line) => [line.path, line.start, line.end, line.silent]), [["App.tsx", 8, 20, false], ["App.tsx", 40, 44, true]]);
  // `all`: any silence names a move; a decision does not
  assert.deepEqual(unnamedUp([{ site: "App.tsx@41-43|SC1002" }, { site: "App.tsx@10-13|SC1001" }], lines, new Set(["all"])).map((move) => move.site), ["App.tsx@10-13|SC1001"]);
  const up = [{ site: "App.tsx@10-13|SC1001", from: "U", to: "clean" }, { site: "App.tsx@41-43|SC1002", from: "U", to: "clean" }, { site: "App.tsx@60-64|SC1001", from: "U", to: "V" }, { site: "Other.tsx@10-13|SC1001", from: "U", to: "clean" }];
  assert.deepEqual(unnamedUp(up, lines, new Set(["callee_roles.new-arm", "after_await_accounting.account.new-reason"])).map((move) => move.site), ["App.tsx@60-64|SC1001", "Other.tsx@10-13|SC1001"]);
  // an arm the change does not name explains nothing
  assert.equal(unnamedUp(up.slice(0, 1), lines, new Set(["callee_roles.other"])).length, 1);
});
