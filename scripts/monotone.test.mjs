import assert from "node:assert/strict";
import { test } from "vitest";

import { identity, moves, siteCodes } from "./monotone.mjs";

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
