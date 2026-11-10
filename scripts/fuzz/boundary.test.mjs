// The boundary registry's coverage test (ADR 0935). A silence is only as sound as the premise of the item that gave it; the registry (boundary.mjs)
// holds, for every item of the checker that can make a site silent, the premise and a pair of programs that differ only in the fact it claims. This
// test fails when an item has neither a pair or a fixture nor a stated gap, when a stated gap is stale, and when a pair names something the generator
// does not have. The lists are derived from the Rust source (boundary-inventory.mjs): adding an arm, a named end, a premise kind or a table row to the
// checker fails this test until the registry covers it or states why not.
//
// Not required, each stated here and in ADR 0935:
//   `Unknown` arms    an obligation (an uncertifiable finding), never a clean
//   reporting arms    a wrong answer is a false positive, which the differential grids' own judge finds
//   owner arms        ADR 0886's `ARM_ENDINGS` (owner_emission.rs) maps each to an ending
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, test } from "vitest";

import { ARM_GAPS, END_GAPS, KIND_GAPS, TABLE_GAP_GROUPS } from "./boundary-gaps.mjs";
import { DIALECT_SOURCES, DIALECT_VOCABULARY, DISPATCH_PAIRS, EXPECTED_UNEXECUTED, PREMISES, WITNESSES, allPairs, coveredArms, dialectClassification, pairCells, problemsOf, resolvePair, sideOf, snapshotOf, tablePairs } from "./boundary.mjs";
import { REPO, deferrals, dialectQuestions, dropPasses, namedEnds, premiseKinds, silentArms, tableEntries, taggedArms } from "./boundary-inventory.mjs";
import { NOT_SEAMS, SEAMS } from "./boundary-seams.mjs";
import { SPEC } from "./misuse.mjs";
import { TABLE_GAPS, TABLE_ROWS, tableContexts } from "./table-shapes.mjs";
import { fileURLToPath } from "node:url";

const snapshot = JSON.parse(readFileSync(fileURLToPath(WITNESSES), "utf8"));
const premises = Object.values(PREMISES);
const pairs = allPairs();
const pairIds = new Set(pairs.map((pair) => pair.id));

describe("the inventories (derived from the Rust source)", () => {
  test("are not empty: an extractor that silently returns nothing proves nothing", () => {
    assert.ok(taggedArms().length > 200, "tagged arms");
    assert.ok(silentArms().length > 100, "silent-capable arms");
    assert.ok(namedEnds().length >= 30, "named ends");
    assert.ok(premiseKinds().length >= 35, "premise kinds");
    assert.ok(dropPasses().length >= 14, "drop passes");
    assert.ok(tableEntries().length > 250, "table rows");
    assert.ok(dialectQuestions().length > 120, "dialect questions");
  });

  test("leave out only the arms the header states: Unknown, reporting and owner arms", () => {
    const kinds = new Set(taggedArms().map((arm) => arm.kind));
    assert.deepEqual([...kinds].sort(), ["computed", "owner", "read-silent", "reporting", "unknown", "write-silent-only"]);
    const silent = new Set(silentArms().map((arm) => arm.id));
    for (const arm of taggedArms()) assert.equal(silent.has(arm.id), ["read-silent", "write-silent-only", "computed"].includes(arm.kind), arm.id);
  });
});

describe("the pairs", () => {
  test("name contexts and codes the generator has, and have both sides for some code (or state why not)", () => {
    for (const pair of pairs) {
      for (const code of pair.codes) assert.ok(SPEC[code], `${pair.id}: no code ${code}`);
      if (pair.oneSided) { assert.ok(pair.oneSided.length > 30, `${pair.id}: the reason is stated`); continue; }
      assert.ok(resolvePair(pair).length > 0, `${pair.id}: no code has a misuse and a legal context among ${pair.contexts.join(", ")}`);
    }
  });

  test("every context of a pair is a context of the generator, and on a side for some code of the grid", () => {
    const known = new Set(Object.values(SPEC).flatMap((spec) => [...(spec.contexts ?? []), ...(spec.legal ?? []), ...(spec.recallContexts ?? []), ...(spec.recallLegal ?? [])]));
    for (const pair of pairs) {
      for (const context of pair.contexts) assert.ok(known.has(context), `${pair.id}: the generator has no context ${context}`);
      assert.ok(pair.contexts.some((context) => pair.codes.some((code) => sideOf(code, context))), `${pair.id}: none of its contexts is on a side of its codes`);
    }
  });

  test("have unique ids and cells", () => {
    assert.equal(pairIds.size, pairs.length);
    for (const pair of pairs) assert.ok(pairCells(pair).length > 0 || pair.oneSided, pair.id);
  });

  test("of the table family: one set per row, from the variants its kind has", () => {
    const contexts = new Set(tableContexts().map((entry) => entry.context));
    const rows = tablePairs();
    for (const pair of rows) for (const context of pair.contexts) assert.ok(contexts.has(context), `${pair.id}: ${context}`);
    for (const row of TABLE_ROWS) assert.ok(rows.some((pair) => pair.row === row.name && pair.kind === row.kind), `${row.name} (${row.kind}) has no pair`);
  });
});

describe("the premises", () => {
  test("each states its claim, and has a pair, a fixture, an axis or a stated reason", () => {
    for (const premise of premises) {
      assert.ok(premise.claim?.length > 20, `${premise.id}: claim`);
      const evidence = premise.pairs.length + premise.fixtures.length + (premise.outside ? 1 : 0) + (premise.axes ? 1 : 0) + (premise.tablePairs?.length ?? 0);
      assert.ok(evidence > 0, `${premise.id}: no evidence`);
      for (const id of premise.pairs) assert.ok(pairIds.has(id), `${premise.id}: no pair ${id}`);
      for (const fixture of premise.fixtures) assert.ok(existsSync(join(REPO, "fixtures", fixture)), `${premise.id}: no fixture ${fixture}`);
      if (premise.outside) assert.ok(premise.outside.length > 30, `${premise.id}: the reason is stated`);
    }
  });

  test("never list the same kind twice", () => {
    const seen = new Map();
    for (const premise of premises) for (const kind of premise.kinds) {
      assert.ok(!seen.has(kind), `${kind}: ${seen.get(kind)} and ${premise.id}`);
      seen.set(kind, premise.id);
    }
  });
});

describe("premise kinds, drop passes and named ends", () => {
  const covered = new Set(premises.flatMap((premise) => premise.kinds));
  test("every PremiseKind has a premise or a stated gap, and no gap is stale", () => {
    const all = premiseKinds().map((kind) => kind.id);
    for (const kind of all) assert.ok(covered.has(kind) || KIND_GAPS[kind], `${kind}: no premise and no gap`);
    for (const [kind, reason] of Object.entries(KIND_GAPS)) {
      assert.ok(all.includes(kind), `stale gap: ${kind} is not a PremiseKind`);
      assert.ok(!covered.has(kind), `stale gap: ${kind} has a premise now`);
      assert.ok(reason.length > 40, `${kind}: the reason is stated`);
    }
    for (const kind of covered) assert.ok(all.includes(kind), `a premise names ${kind}, which is not a PremiseKind`);
  });

  test("every DropPass rests on kinds the registry knows", () => {
    for (const pass of dropPasses()) {
      assert.ok(pass.premises.length > 0, `${pass.id}: premise_kinds`);
      for (const kind of pass.premises) assert.ok(covered.has(kind) || KIND_GAPS[kind], `${pass.id} rests on ${kind}: no premise and no gap`);
    }
  });

  test("every named end has a premise or a stated gap, and no gap is stale", () => {
    const listed = new Set(premises.flatMap((premise) => premise.ends));
    const all = namedEnds().map((end) => end.id);
    for (const end of all) assert.ok(listed.has(end) || END_GAPS[end], `${end}: no premise and no gap`);
    for (const [end, reason] of Object.entries(END_GAPS)) {
      assert.ok(all.includes(end), `stale gap: ${end} is not a named end`);
      assert.ok(!listed.has(end), `stale gap: ${end} has a premise now`);
      assert.ok(reason.length > 40, `${end}: the reason is stated`);
    }
    for (const end of listed) assert.ok(all.includes(end), `a premise names ${end}, which is not a named end`);
  });
});

describe("the tables", () => {
  const FAMILY_TABLES = new Set(["argument-behavior", "fresh-stack-schedulers", "carrier-runs", "callback-host"]);
  const rowNames = new Set(TABLE_ROWS.map((row) => row.name));
  const stated = new Map(TABLE_GAP_GROUPS.flatMap((group) => group.rows.map((row) => [row, group.reason])));

  test("every row has a shape of the table family, a dispatch pair, a stated gap of its own file, or a stated group gap", () => {
    for (const entry of tableEntries()) {
      const key = `${entry.table}:${entry.name}`;
      const family = FAMILY_TABLES.has(entry.table) && (rowNames.has(entry.name) || TABLE_GAPS[entry.name]) && !stated.has(key);
      const dispatch = DISPATCH_PAIRS[key]?.every((id) => pairIds.has(id));
      assert.ok(family || dispatch || stated.has(key), `${key}: no shape and no gap`);
    }
  });

  test("no group gap is stale: its row exists, and nothing else covers it", () => {
    const keys = new Set(tableEntries().map((entry) => `${entry.table}:${entry.name}`));
    for (const group of TABLE_GAP_GROUPS) {
      assert.ok(group.reason.length > 40, "the reason is stated");
      for (const row of group.rows) {
        assert.ok(keys.has(row), `stale gap: ${row} is not a table row`);
        assert.ok(!DISPATCH_PAIRS[row], `stale gap: ${row} has a dispatch pair`);
      }
    }
  });

  test("the family's own gaps (rows no program can run) state why", () => {
    for (const [name, reason] of Object.entries(TABLE_GAPS)) assert.ok(reason.length > 40, `${name}: the reason is stated`);
  });

  test("dispatch pairs name pairs that exist", () => {
    for (const [key, ids] of Object.entries(DISPATCH_PAIRS)) for (const id of ids) assert.ok(pairIds.has(id), `${key}: no pair ${id}`);
  });
});

describe("the dialect questions", () => {
  test("each is vocabulary, a source question or a role question, and the classes are disjoint and real", () => {
    const questions = dialectQuestions();
    const names = new Set(questions.map((question) => question.id));
    for (const name of [...Object.keys(DIALECT_VOCABULARY), ...Object.keys(DIALECT_SOURCES)]) assert.ok(names.has(name), `${name} is not a dialect question`);
    for (const name of Object.keys(DIALECT_VOCABULARY)) assert.ok(!DIALECT_SOURCES[name], `${name} is in both classes`);
    const classes = dialectClassification(questions);
    assert.ok(classes.filter((entry) => entry.kind === "role").length > 50, "the role questions");
    for (const entry of classes) if (entry.premise) assert.ok(PREMISES[entry.premise], entry.premise);
  });
});

describe("the arms", () => {
  const covered = coveredArms(snapshot);
  const all = silentArms().map((arm) => arm.id);
  const gaps = new Map(ARM_GAPS.flatMap((group) => group.arms.map((arm) => [arm, group.reason])));

  test("every silent-capable arm is separated by a pair (the snapshot) or has a stated gap", () => {
    for (const arm of all) assert.ok(covered.has(arm) || gaps.has(arm), `${arm}: no pair separates it and no gap states why (node scripts/fuzz/boundary.mjs verify --update)`);
  });

  test("no gap is stale: the arm exists, no pair separates it now", () => {
    for (const [arm, reason] of gaps) {
      assert.ok(all.includes(arm), `stale gap: ${arm} is not a silent-capable arm`);
      assert.ok(!covered.has(arm), `stale gap: ${arm} is separated by ${covered.get(arm)?.[0]?.pair}`);
      assert.ok(reason.length > 40, `${arm}: the reason is stated`);
    }
    for (const group of ARM_GAPS) assert.equal(new Set(group.arms).size, group.arms.length, "an arm twice in one group");
    assert.equal(gaps.size, ARM_GAPS.reduce((sum, group) => sum + group.arms.length, 0), "an arm in two groups");
  });

  test("the snapshot is the registry's: its pairs, contexts and codes exist", () => {
    for (const id of Object.keys(snapshot.pairs)) assert.ok(pairIds.has(id), `snapshot pair ${id} is not in the registry`);
    for (const [arm, witnesses] of Object.entries(snapshot.arms)) {
      assert.ok(all.includes(arm), `snapshot arm ${arm} is not a silent-capable arm`);
      for (const witness of witnesses) {
        assert.ok(pairIds.has(witness.pair), `${arm}: pair ${witness.pair}`);
        assert.ok(sideOf(witness.code, witness.context) === "legal", `${arm}: ${witness.code} ${witness.context} is not a legal cell`);
      }
    }
    for (const pair of pairs) if (!pair.oneSided) assert.ok(snapshot.pairs[pair.id], `the snapshot has no run of pair ${pair.id}: node scripts/fuzz/boundary.mjs verify --update`);
  });

  test("a witness counts only where the misuse side of its pair, for that code, left no program clean", () => {
    const fake = snapshotOf([
      { pair: "p", code: "C", side: "misuse", context: "a", programOutcome: "uncertifiable", decisions: [] },
      { pair: "p", code: "C", side: "legal", context: "b", programOutcome: "clean", decisions: [{ silent: true, arm: "x.y", default: false }] },
      { pair: "q", code: "C", side: "misuse", context: "a", programOutcome: "clean", decisions: [] },
      { pair: "q", code: "C", side: "legal", context: "b", programOutcome: "clean", decisions: [{ silent: true, arm: "x.z", default: false }] },
    ]);
    assert.deepEqual([...coveredArms(fake).keys()], ["x.y"]);
  });
});

describe("what a run reports", () => {
  test("a violation at a site that never runs is kept apart from the candidate false positives, by one predicate (owner ruling 2026-11-10)", () => {
    const row = (pair, code, side, programOutcome, id) => ({ pair, code, side, programOutcome, id });
    const rows = [
      row("table:Array.forEach:b/zero", "REACTIVE_WRITE_IN_OWNED_SCOPE", "legal", "violation", "zero-write"),
      row("table:Array.forEach:b/zero", "ACTION_CALLED_IN_OWNED_SCOPE", "legal", "violation", "zero-action"),
      row("table:Array.forEach:b/h", "REACTIVE_WRITE_IN_OWNED_SCOPE", "legal", "violation", "handler-write"),
      row("table:Array.forEach:b/zero", "STRICT_READ_UNTRACKED", "legal", "violation", "zero-read"),
      row("table:Array.forEach:b/zero", "REACTIVE_WRITE_IN_OWNED_SCOPE", "misuse", "clean", "misuse-silent"),
    ];
    const { silentMisuse, reportedLegal, keptUnexecuted } = problemsOf(rows);
    assert.deepEqual(keptUnexecuted.map((entry) => entry.id), ["zero-write", "zero-action"]);
    // A read at such a site is silent (ADR 0563), so a reported read there is a candidate false positive like any other.
    assert.deepEqual(reportedLegal.map((entry) => entry.id), ["handler-write", "zero-read"]);
    assert.deepEqual(silentMisuse.map((entry) => entry.id), ["misuse-silent"]);
    assert.match(EXPECTED_UNEXECUTED.ruling, /2026-11-10/);
  });
});

describe("the seams", () => {
  const hits = deferrals();
  const at = (entry, hit) => hit.file.startsWith(entry.file) && entry.match.test(hit.text);
  test("a comment that hands a case to another check is a seam or states why it is not", () => {
    assert.ok(hits.length > 20, "an extractor that finds nothing proves nothing");
    for (const hit of hits) {
      const named = SEAMS.filter((seam) => seam.sites.some((site) => at(site, hit))).length + NOT_SEAMS.filter((entry) => at(entry, hit)).length;
      assert.equal(named, 1, `${hit.file}:${hit.line} "${hit.text}" is in ${named} list(s): add it to SEAMS (the two checks, the case between them, the evidence) or to NOT_SEAMS (the reason)`);
    }
  });

  test("no entry is stale: each matches a comment now in the source", () => {
    for (const seam of SEAMS) for (const site of seam.sites) assert.ok(hits.some((hit) => at(site, hit)), `seam ${seam.id}: no comment matches ${site.match}`);
    for (const entry of NOT_SEAMS) assert.ok(hits.some((hit) => at(entry, hit)), `NOT_SEAMS ${entry.file}: no comment matches ${entry.match}`);
  });

  test("a seam has evidence that exists (a pair, a fixture) or a stated gap, never both", () => {
    const ids = new Set();
    for (const seam of SEAMS) {
      assert.ok(!ids.has(seam.id), `duplicate seam ${seam.id}`);
      ids.add(seam.id);
      assert.equal(seam.between?.length, 2, `${seam.id}: names the two checks`);
      assert.ok(seam.case?.length > 30, `${seam.id}: states the case that falls between them`);
      const evidence = (seam.pairs?.length ?? 0) + (seam.fixtures?.length ?? 0);
      assert.ok(Boolean(evidence) !== Boolean(seam.gap), `${seam.id}: evidence (pairs, fixtures) or a gap, not both and not neither`);
      for (const pair of seam.pairs ?? []) assert.ok(pairIds.has(pair), `${seam.id}: no pair ${pair}`);
      for (const fixture of seam.fixtures ?? []) assert.ok(existsSync(join(REPO, "fixtures", fixture)), `${seam.id}: no fixture ${fixture}`);
      if (seam.gap) assert.ok(seam.gap.length > 40, `${seam.id}: the gap states why`);
    }
  });
});
