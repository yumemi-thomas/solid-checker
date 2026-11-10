import assert from "node:assert/strict";
import { describe, test } from "vitest";

import { armsOf, findingSites, findingsKey, gridPairs, moved, summarize, transitions } from "./proof-ablation.mjs";

const decision = (arm, extra = {}) => ({ k: "decision", t: "read", role: "TrackedJsx", arm, default: false, silent: true, ...extra });

describe("the arms an ablation run tries", () => {
  test("only the tagged, non-default arms that answered a silent decision", () => {
    const lines = [
      decision("callee_roles.tracked-callback"),
      decision("callee_roles.tracked-callback"),
      decision("write_roles.walk-direct-1", { t: "write" }),
      decision("callee_roles.unplaced", { silent: false, role: "Unknown" }),
      decision("callee_roles.fallback", { default: true }),
      decision(null),
      { k: "row", t: "read" },
    ];
    assert.deepEqual(armsOf(lines), ["callee_roles.tracked-callback", "write_roles.walk-direct-1"]);
  });
});

describe("the findings of a run", () => {
  const finding = (id, start) => ({ id, kind: "violation", analysisContext: "", primaryLocation: { path: "/p/App.tsx", startByte: start, endByte: start + 3 } });

  test("compare equal whatever their order, and unequal when a finding moves or goes", () => {
    const base = findingsKey({ findings: [finding("SC1001", 5), finding("SC2001", 9)] });
    assert.equal(findingsKey({ findings: [finding("SC2001", 9), finding("SC1001", 5)] }), base);
    assert.notEqual(findingsKey({ findings: [finding("SC1001", 5)] }), base);
    assert.notEqual(findingsKey({ findings: [finding("SC1001", 6), finding("SC2001", 9)] }), base);
  });
});

describe("the ablation report", () => {
  test("an arm is witnessed by the fixtures whose findings changed, unwitnessed when none did", () => {
    const { arms, unwitnessed } = summarize([
      { fixture: "a", arm: "x", changed: true },
      { fixture: "b", arm: "x", changed: false },
      { fixture: "b", arm: "y", changed: false },
    ]);
    assert.deepEqual(arms.x, { witnesses: ["a"], inert: ["b"] });
    assert.deepEqual(unwitnessed, ["y"]);
  });
});

describe("the sites of a run", () => {
  const raw = (id, kind, start, path = "/p/App.tsx", analysisContext = "") => ({ id, kind, analysisContext, primaryLocation: { path, startByte: start, endByte: start + 4 } });

  test("are the findings' spans, with the code and the blocker, and leave the harness entry out", () => {
    const sites = findingSites({ findings: [raw("SC1001", "uncertifiable", 7, "/p/App.tsx", "function-run-unplaced:f"), raw("SC9012", "violation", 3, "/p/main.tsx")] });
    assert.deepEqual(sites, [{ site: "/p/App.tsx:7:11", id: "SC1001", kind: "uncertifiable", blocker: "function-run-unplaced" }]);
  });
});

describe("the transitions of an ablated run", () => {
  const at = (site, id, kind, blocker = "") => ({ site, id, kind, blocker });

  test("a clean site that gains an obligation stands on the arm alone; one that gains a violation is two arms disagreeing", () => {
    const moves = transitions([], [at("s1", "SC1001", "uncertifiable", "function-run-unplaced"), at("s2", "SC2001", "violation")]);
    assert.deepEqual(moves.cleanToU.map((move) => move.site), ["s1"]);
    assert.deepEqual(moves.cleanToU[0].ablated, ["SC1001:function-run-unplaced"]);
    assert.deepEqual(moves.cleanToV.map((move) => move.site), ["s2"]);
  });

  test("a violation that becomes an obligation, or vanishes, is the arm's proof of the violation", () => {
    const moves = transitions([at("s1", "SC1001", "violation"), at("s2", "SC1001", "violation")], [at("s1", "SC1001", "uncertifiable", "x")]);
    assert.deepEqual(moves.vToU.map((move) => move.site), ["s1"]);
    assert.deepEqual(moves.vLost.map((move) => move.site), ["s2"]);
  });

  test("an obligation that becomes a violation or vanishes", () => {
    const moves = transitions([at("s1", "SC1001", "uncertifiable"), at("s2", "SC1001", "uncertifiable")], [at("s1", "SC1001", "violation")]);
    assert.deepEqual(moves.uToV.map((move) => move.site), ["s1"]);
    assert.deepEqual(moves.uLost.map((move) => move.site), ["s2"]);
  });

  test("a site whose kind is unchanged is no transition, whatever code answers there", () => {
    const moves = transitions([at("s1", "SC1001", "violation")], [at("s1", "SC2001", "violation")]);
    assert.equal(moved(moves), false);
    assert.equal(moved(transitions([], [])), false);
  });

  test("the report counts them per arm and keeps every clean to violation", () => {
    const first = transitions([], [at("s1", "SC1001", "uncertifiable"), at("s2", "SC2001", "violation")]);
    const second = transitions([], [at("s3", "SC1001", "violation")]);
    const { arms } = summarize([
      { fixture: "a", arm: "x", changed: true, transitions: first },
      { fixture: "b", arm: "x", changed: true, transitions: second },
      { fixture: "c", arm: "x", changed: false, transitions: transitions([], []) },
    ]);
    assert.equal(arms.x.counts.cleanToU, 1);
    assert.equal(arms.x.counts.cleanToV, 2);
    assert.deepEqual(arms.x.examples.cleanToV.map((example) => `${example.program}:${example.site}`), ["a:s2", "b:s3"]);
    assert.deepEqual(arms.x.witnesses, ["a", "b"]);
    assert.deepEqual(arms.x.inert, ["c"]);
  });
});

describe("a grid's pairs", () => {
  test("come from the silent decisions of its role log, one pair per program and arm, bounded per arm", async () => {
    const { mkdtempSync, writeFileSync, rmSync } = await import("node:fs");
    const { tmpdir } = await import("node:os");
    const { join } = await import("node:path");
    const dir = mkdtempSync(join(tmpdir(), "ablation-grid-"));
    try {
      const file = join(dir, "roles.jsonl");
      const path = (id, program) => ({ k: "path", q: 1, p: id, f: `/w/.solid-checker-fuzz/run-1/slots/0/${program}/App.tsx`, g: program });
      const decide = (id, arm, extra = {}) => ({ k: "read", q: 1, p: id, s: 1, e: 2, role: "DeferredCallback", arm, default: false, silent: true, ...extra });
      writeFileSync(file, [
        path(0, "p1"), path(1, "p2"),
        decide(0, "callee_roles.a"), decide(0, "callee_roles.a"), decide(0, "callee_roles.b"), decide(0, "callee_roles.c", { silent: false }), decide(0, "callee_roles.d", { default: true }),
        decide(1, "callee_roles.a"),
      ].map((line) => JSON.stringify(line)).join("\n") + "\n");
      assert.deepEqual(await gridPairs(file), [
        { program: "p1", arm: "callee_roles.a" }, { program: "p1", arm: "callee_roles.b" }, { program: "p2", arm: "callee_roles.a" },
      ]);
      assert.deepEqual(await gridPairs(file, { maxPerArm: 1 }), [{ program: "p1", arm: "callee_roles.a" }, { program: "p1", arm: "callee_roles.b" }]);
      assert.deepEqual(await gridPairs(file, { arms: ["callee_roles.b"] }), [{ program: "p1", arm: "callee_roles.b" }]);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
