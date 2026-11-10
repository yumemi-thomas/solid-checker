import assert from "node:assert/strict";
import { test } from "vitest";

import { addsViolation, blockerOf, buildReport, diffProject, hazardMatches, keyOf, parseOptions, pickControls, recordsHazards, render, scope } from "./corpus-delta.mjs";

// ADR 0955: the scoped corpus delta, on in-memory results.
const at = (path, startByte, line = 1) => ({ path, startByte, endByte: startByte + 4, line });
const finding = (id, kind, startByte, blocker, extra = {}) => ({ id, kind, primaryLocation: at("/a/x.ts", startByte, startByte), ...(blocker ? { blockedBy: { kind: blocker } } : {}), ...extra });
const row = (app, project) => ({ app, project });
const result = (app, project, findings, hazards) => ({ ...row(app, project), exitCode: 0, parseError: null, findings, ...(hazards ? { hazards } : {}) });
const table = (...ids) => ids.map((id) => ({ id, sites: [] }));

test("a blocker is the typed kind, else the head of the context, else none", () => {
  assert.equal(blockerOf({ blockedBy: { kind: "standard-member-impostor" }, analysisContext: "other" }), "standard-member-impostor");
  assert.equal(blockerOf({ analysisContext: "function-run-unplaced: more" }), "function-run-unplaced");
  assert.equal(blockerOf({}), "none");
});

test("a hazard is named by its id or its head", () => {
  assert.equal(hazardMatches("OpenMemberWrite(*)", ["OpenMemberWrite"]), true);
  assert.equal(hazardMatches("OpenMemberWrite(key)", ["OpenMemberWrite"]), true);
  assert.equal(hazardMatches("OpenMemberWrite(*)", ["OpenMemberWrite(*)"]), true);
  assert.equal(hazardMatches("StandardRootHanded", ["OpenMemberWrite"]), false);
  assert.equal(hazardMatches("OpenMemberWriteExtra", ["OpenMemberWrite"]), false);
});

const results = new Map([
  [keyOf(row("kui", "a")), result("kui", "a", [finding("SC9012", "uncertifiable", 1, "standard-member-impostor"), finding("SC1001", "uncertifiable", 9, "function-run-unplaced")], table("OpenMemberWrite(*)", "StandardRootHanded"))],
  [keyOf(row("kui", "b")), result("kui", "b", [finding("SC1001", "uncertifiable", 1, "function-run-unplaced"), finding("SC1001", "uncertifiable", 2, "function-run-unplaced"), finding("SC1001", "uncertifiable", 3, "function-run-unplaced")], table("RootPatch"))],
  [keyOf(row("game", "c")), result("game", "c", [finding("SC2003", "violation", 1)])],
  [keyOf(row("game", "d")), result("game", "d", [finding("SC1001", "uncertifiable", 1, "function-run-unplaced")], table("StandardRootHanded"))],
]);

test("the scope is the projects whose table holds a hazard the change reads, or a row of a blocker it names", () => {
  const byHazard = scope(results, { hazards: ["OpenMemberWrite", "StandardRootHanded"], blockers: [], all: false });
  assert.deepEqual([...byHazard.affected], [[keyOf(row("kui", "a")), "hazards OpenMemberWrite(*), StandardRootHanded"], [keyOf(row("game", "d")), "hazards StandardRootHanded"]]);
  assert.deepEqual(byHazard.unaffected, [keyOf(row("kui", "b")), keyOf(row("game", "c"))]);
  const byBlocker = scope(results, { hazards: [], blockers: ["function-run-unplaced"], all: false });
  assert.deepEqual([...byBlocker.affected.keys()], [keyOf(row("kui", "a")), keyOf(row("kui", "b")), keyOf(row("game", "d"))]);
  assert.deepEqual(scope(results, { hazards: [], blockers: [], all: true }).unaffected, []);
});

test("a project with no table is none for a build that records hazards and unknown for one that does not", () => {
  // game/c has no table: the build records hazards (the others have tables), so it holds none
  assert.equal(recordsHazards(results), true);
  assert.equal(scope(results, { hazards: ["RootPatch"], blockers: [], all: false }).affected.has(keyOf(row("game", "c"))), false);
  // an old build with neither a table nor a typed blocker anywhere: nothing can be said, every project is in
  const bare = new Map([[keyOf(row("g", "x")), result("g", "x", [finding("SC1001", "uncertifiable", 1)])], [keyOf(row("g", "y")), result("g", "y", [])]]);
  assert.equal(recordsHazards(bare), false);
  const unknown = scope(bare, { hazards: ["RootPatch"], blockers: [], all: false });
  assert.deepEqual([...unknown.affected.values()], ["the old build records no hazards table", "the old build records no hazards table"]);
});

test("the controls are the unaffected projects with the most uncertifiable rows, ties by name", () => {
  const unaffected = [keyOf(row("kui", "b")), keyOf(row("game", "c")), keyOf(row("zed", "z"))];
  const more = new Map([...results, [keyOf(row("zed", "z")), result("zed", "z", [finding("SC1001", "uncertifiable", 1), finding("SC1001", "uncertifiable", 2), finding("SC1001", "uncertifiable", 3)])]]);
  assert.deepEqual(pickControls(more, unaffected, 2), [keyOf(row("kui", "b")), keyOf(row("zed", "z"))]);
  assert.deepEqual(pickControls(more, unaffected, 0), []);
});

test("a project's moves are rows that are new, gone or changed kind, and its counts are by kind and blocker", () => {
  const before = [finding("SC9012", "violation", 1), finding("SC9012", "uncertifiable", 10, "standard-member-impostor"), finding("SC1001", "uncertifiable", 20, "function-run-unplaced")];
  const after = [finding("SC9012", "uncertifiable", 1, "standard-member-impostor"), finding("SC9012", "uncertifiable", 10, "standard-member-impostor"), finding("SC2001", "violation", 30)];
  const delta = diffProject(before, after);
  assert.deepEqual([delta.old.violation, delta.old.uncertifiable, delta.new.violation, delta.new.uncertifiable], [1, 2, 1, 2]);
  assert.deepEqual(delta.new.blockers, { "standard-member-impostor": 2 });
  assert.deepEqual(delta.moves.map(({ rule, from, to, blocker }) => [rule, from, to, blocker]), [
    ["SC1001", "uncertifiable", "gone", "function-run-unplaced"],
    ["SC2001", "new", "violation", "none"],
    ["SC9012", "violation", "uncertifiable", "standard-member-impostor"],
  ]);
  assert.deepEqual(delta.moves.map(addsViolation), [false, true, false]);
});

test("the report totals the moves, names what needs a look, and fails on an added violation or a moved control", () => {
  const oldResults = new Map([
    [keyOf(row("kui", "a")), result("kui", "a", [finding("SC9012", "violation", 1)])],
    [keyOf(row("kui", "b")), result("kui", "b", [finding("SC1001", "uncertifiable", 1, "function-run-unplaced")])],
  ]);
  const newResults = new Map([
    [keyOf(row("kui", "a")), result("kui", "a", [finding("SC9012", "uncertifiable", 1, "standard-member-impostor")])],
    [keyOf(row("kui", "b")), result("kui", "b", [])],
  ]);
  const affected = new Map([[keyOf(row("kui", "a")), "hazards OpenMemberWrite(*)"]]);
  const report = buildReport({ oldResults, newResults, affected, controls: [keyOf(row("kui", "b"))] });
  assert.deepEqual(report.totals, { old: { violation: 1, uncertifiable: 1 }, new: { violation: 0, uncertifiable: 1 } });
  assert.deepEqual(report.blockers, [{ kind: "function-run-unplaced", old: 1, new: 0 }, { kind: "standard-member-impostor", old: 0, new: 1 }]);
  assert.deepEqual(report.needsLook, ["control kui/b moved 1 rows: the scope missed a hazard"]);
  const added = buildReport({
    oldResults: new Map([[keyOf(row("kui", "a")), result("kui", "a", [finding("SC9012", "uncertifiable", 1, "standard-member-impostor")])]]),
    newResults: new Map([[keyOf(row("kui", "a")), result("kui", "a", [finding("SC9012", "violation", 1)])]]),
    affected, controls: [],
  });
  assert.deepEqual(added.needsLook, ["kui/a: SC9012 uncertifiable -> violation at line 1 (none)"]);
  const clean = buildReport({ oldResults, newResults: new Map([[keyOf(row("kui", "a")), newResults.get(keyOf(row("kui", "a")))]]), affected, controls: [] });
  assert.deepEqual(clean.needsLook, []);
  const errored = buildReport({ oldResults, newResults, affected, controls: [], errors: ["new build, kui/a: killed (timeout)"] });
  assert.ok(errored.needsLook.includes("new build, kui/a: killed (timeout)"));
});

test("the text report is deterministic and says what moved", () => {
  const oldResults = new Map([[keyOf(row("kui", "a")), result("kui", "a", [finding("SC9012", "violation", 1)])]]);
  const newResults = new Map([[keyOf(row("kui", "a")), result("kui", "a", [finding("SC9012", "uncertifiable", 1, "standard-member-impostor")])]]);
  const report = buildReport({ oldResults, newResults, affected: new Map([[keyOf(row("kui", "a")), "hazards OpenMemberWrite(*)"]]), controls: [] });
  const text = render(report, ["corpus delta: old a, new b"]);
  assert.equal(text, render(JSON.parse(JSON.stringify(report)), ["corpus delta: old a, new b"]));
  assert.match(text, /kui\/a \(hazards OpenMemberWrite\(\*\)\)/);
  assert.match(text, /standard-member-impostor/);
  assert.match(text, /SC9012  violation -> uncertifiable  \(standard-member-impostor\)/);
  assert.match(text, /needs a look: nothing/);
});

test("the options must name the scope and both builds", () => {
  const base = ["--old", "/a", "--new", "/b"];
  assert.throws(() => parseOptions(base), /name what the change reads/);
  assert.throws(() => parseOptions(["--new", "/b", "--all"]), /--old <checker> is required/);
  assert.throws(() => parseOptions([...base, "--all", "--hazard", "X"]), /takes no --hazard or --blocker/);
  assert.throws(() => parseOptions([...base, "--hazard", "X", "--controls", "-1"]), /count/);
  assert.throws(() => parseOptions([...base, "--hazard", "X", "--bogus", "1"]), /unknown or incomplete option/);
  const parsed = parseOptions([...base, "--hazard", "OpenMemberWrite,StandardRootHanded", "--blocker", "standard-member-impostor", "--json"]);
  assert.deepEqual(parsed.spec, { hazards: ["OpenMemberWrite", "StandardRootHanded"], blockers: ["standard-member-impostor"], all: false });
  assert.equal(parsed.json, true);
  assert.equal(parsed.controls, 3);
  assert.equal(parseOptions([...base, "--all"]).spec.all, true);
});
