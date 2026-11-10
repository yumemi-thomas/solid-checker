import assert from "node:assert/strict";
import { describe, test } from "vitest";

import { armsOf, findingsKey, summarize } from "./proof-ablation.mjs";

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
