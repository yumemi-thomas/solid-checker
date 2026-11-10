import assert from "node:assert/strict";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { test } from "vitest";

import { countRows } from "./guard/count.mjs";
import { analyze, handedRootHolds, liftingSets, render } from "./u-blockers.mjs";

// ADR 0947: the blocker report, on in-memory checker reports.
const row = (blockedBy) => ({ kind: "uncertifiable", id: "SC9012", blockedBy });
const handed = (fired) => row({ kind: "standard-member-impostor", scope: "program", formula: "handed-root-patch", fired });
const FIRED = ["OpenMemberWrite(*)", "StandardRootHanded"];
const site = (path, start) => ({ path, start, end: start + 1 });
const reportA = {
  findings: [handed(FIRED), handed(FIRED), row({ kind: "function-run-unplaced" }), { kind: "violation", id: "SC1001" }],
  hazards: [
    { id: "OpenMemberWrite(*)", sites: [site("/a/x.ts", 10), site("/a/y.ts", 4)] },
    { id: "StandardRootHanded", sites: [] },
  ],
};
const reportB = {
  findings: [
    handed(["OpenMemberWrite(*)"]),
    row({ kind: "tuple-iteration-written", scope: "program", formula: "array-slots-withdrawn", fired: ["VisibleIterationWrite"] }),
    row({ kind: "provider-not-proven-above", missing: { fact: "provider-above", member: "Ctx" } }),
  ],
  hazards: [{ id: "OpenMemberWrite(*)", sites: [site("/a/x.ts", 10)] }, { id: "VisibleIterationWrite", sites: [] }],
};

test("rows are grouped by kind, with the share that has no located alternative", () => {
  const result = analyze([reportA, reportB]);
  assert.equal(result.rows, 6);
  assert.deepEqual(
    result.kinds.map(({ kind, rows, unlocated }) => [kind, rows, unlocated]),
    [
      ["standard-member-impostor", 3, 0],
      ["function-run-unplaced", 1, 1],
      ["provider-not-proven-above", 1, 1],
      ["tuple-iteration-written", 1, 1],
    ],
  );
});

test("a veto is grouped by its formula and its fired set, never by one hazard", () => {
  const result = analyze([reportA, reportB]);
  assert.deepEqual(
    result.formulas.map(({ formula, fired, rows }) => [formula, fired, rows]),
    [
      ["handed-root-patch", FIRED, 2],
      // Ties by key: the kind first.
      ["handed-root-patch", ["OpenMemberWrite(*)"], 1],
      ["array-slots-withdrawn", ["VisibleIterationWrite"], 1],
    ],
  );
});

test("the minimal lifting sets of the handed-root formula are the written side or the handed side", () => {
  assert.deepEqual(liftingSets("handed-root-patch", FIRED), [["OpenMemberWrite(*)"], ["StandardRootHanded"]]);
  // A write alone holds nothing; nothing to lift.
  assert.deepEqual(liftingSets("handed-root-patch", ["OpenMemberWrite(*)"]), []);
  // Received needs a root patch to count, and an open write of `<symbol>` needs `Symbol` replaceable to be written.
  assert.equal(handedRootHolds(new Set(["OpenMemberWrite(x)", "StandardRootReceived"])), false);
  assert.equal(handedRootHolds(new Set(["OpenMemberWrite(x)", "StandardRootReceived", "RootPatch"])), true);
  assert.equal(handedRootHolds(new Set(["OpenMemberWrite(<symbol>)", "StandardRootHanded"])), false);
  assert.equal(handedRootHolds(new Set(["OpenMemberWrite(<symbol>)", "StandardPatch(.*)", "StandardRootHanded"])), true);
  // A formula the report does not know lifts only when every fired atom goes.
  assert.deepEqual(liftingSets("impostor-veto", ["StandardImpostor(map)", "StandardImpostor(*)"]), [["StandardImpostor(*)", "StandardImpostor(map)"]]);
});

test("a site ranks by the rows whose alternatives name its hazard, ties by key, and the result does not depend on report order", () => {
  const forward = analyze([reportA, reportB]);
  const reverse = analyze([reportB, reportA]);
  assert.deepEqual(forward, reverse);
  assert.deepEqual(
    forward.locations.map(({ hazard, site, rows }) => [hazard, site, rows]),
    [
      ["OpenMemberWrite(*)", "/a/x.ts@10", 3],
      ["OpenMemberWrite(*)", "/a/y.ts@4", 2],
    ],
  );
  assert.equal(analyze([reportA, reportB], { top: 1 }).locations.length, 1);
});

test("a hazard on an alternative reports its sites, its rows and the rows it lifts alone", () => {
  assert.deepEqual(analyze([reportA, reportB]).hazards, [
    { hazard: "OpenMemberWrite(*)", sites: 2, rows: 3, alone: 3 },
    { hazard: "StandardRootHanded", sites: 0, rows: 2, alone: 2 },
    { hazard: "VisibleIterationWrite", sites: 0, rows: 1, alone: 1 },
  ]);
});

test("a hazard present but on no alternative decides nothing and ranks no site", () => {
  // `StandardImpostor(*)` is read only beside `StandardRootIterated`: here it is present and irrelevant.
  const decoy = {
    findings: [handed(["OpenMemberWrite(*)", "RootPatch", "StandardImpostor(*)", "StandardRootHanded", "StandardRootReceived"])],
    hazards: [
      { id: "OpenMemberWrite(*)", sites: [site("/a/x.ts", 10)] },
      { id: "StandardImpostor(*)", sites: [site("/a/decoy.ts", 99)] },
      { id: "RootPatch", sites: [] },
      { id: "StandardRootHanded", sites: [] },
      { id: "StandardRootReceived", sites: [] },
    ],
  };
  const result = analyze([decoy]);
  assert.deepEqual(result.locations.map(({ site }) => site), ["/a/x.ts@10"]);
  assert.deepEqual(result.hazards.map(({ hazard }) => hazard), ["OpenMemberWrite(*)", "RootPatch", "StandardRootHanded", "StandardRootReceived"]);
  assert.equal(result.hazards[0].alone, 1);
  assert.equal(result.kinds[0].unlocated, 0);
});

test("missing is grouped by its fact and member, and a report with none says so", () => {
  assert.deepEqual(analyze([reportB]).missing, [{ kind: "provider-not-proven-above", fact: "provider-above", member: "Ctx", rows: 1 }]);
  assert.match(render(analyze([reportA])), /none: no emitter fills `missing` yet/);
});

// The review of this field (fable-u-review §1): the snapshot's `blocker` is a string the one-way count keys on. If the object
// reached it under that name, every key would read `SCxxxx [object Object]` and the ledger would collapse into a rename failure
// nobody can read. The name `blockedBy` is the object's; the string stays produced from `analysisContext` by coverage.mjs only.
test("the snapshot blocker string is produced by coverage.mjs from analysisContext alone, and no script reads blockedBy into it", () => {
  const scripts = new URL(".", import.meta.url).pathname;
  const sources = [];
  const walk = (directory) => {
    for (const name of readdirSync(directory)) {
      const path = join(directory, name);
      if (statSync(path).isDirectory()) {
        if (!["fuzz", "flip", "node_modules"].includes(name)) walk(path);
      } else if (name.endsWith(".mjs") && !name.endsWith(".test.mjs")) sources.push(path);
    }
  };
  walk(scripts);
  const coverage = readFileSync(join(scripts, "coverage.mjs"), "utf8");
  assert.match(coverage, /\{ blocker: String\(finding\.analysisContext \?\? ""\)\.split\(":"\)\[0\] \}/);
  assert.doesNotMatch(coverage, /blockedBy/);
  for (const path of sources) {
    const text = readFileSync(path, "utf8");
    if (relative(scripts, path) === "u-blockers.mjs") continue;
    assert.doesNotMatch(text, /blocker\s*:[^,\n}]*blockedBy|\.blocker\s*=[^;\n]*blockedBy|blockedBy[^;\n]*\bblocker\s*:/, relative(scripts, path));
  }
  // And the one-way count reads the string, whatever a finding also carries.
  assert.deepEqual(
    countRows([{ findings: [{ code: "SC9012", kind: "uncertifiable", blocker: "standard-member-impostor", blockedBy: { kind: "standard-member-impostor" } }] }]),
    { "SC9012 standard-member-impostor": 1 },
  );
});
