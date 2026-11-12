import assert from "node:assert/strict";
import { mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, test } from "vitest";

import { ROOT } from "./proof-gate.mjs";
import { fixtureProjects } from "./lib/fixture-projects.mjs";
import { EMPTY_REASONS, SITE_CODE, gapProblems, hitKey, proofHits, proofLogLines, proofStats, pruneCandidates, resolvedLine } from "./lib/proof-log.mjs";

const F = "/p/App.tsx";
const at = (s, e = s + 3) => ({ f: F, s, e });
const decision = (s, extra = {}) => ({ k: "decision", ...at(s), t: "read", role: "UntrackedRendering", arm: "callee_roles.inside-component", arms: [], default: false, silent: false, ...extra });
// The writer sets the code of every site line: the table's first (`SITE_CODE`) unless the line is another code of the site.
const site = (s, extra = {}) => ({ k: "site", ...at(s), t: "read", code: SITE_CODE[extra.t ?? "read"], source: "accessor", via: "call", end: "none", detail: "", ...extra });
const row = (s, extra = {}) => ({ k: "row", ...at(s), t: "read", role: "TrackedJsx", reported: false, why: "role-does-not-report", ...extra });
const finding = (s, extra = {}) => ({ k: "finding", ...at(s), code: "SC1001", kind: "violation", ctx: "", ...extra });
const verdict = (s, extra = {}) => ({ ...finding(s, extra), k: "verdict" });
const drop = (s, extra = {}) => ({ k: "drop", ...at(s), code: "SC1001", from: "violation", ctx: "", pass: "ExecutionPaths", premises: [], ...extra });
const silent = (s, extra = {}) => ({ k: "silent", ...at(s), rule: "row:reads", proof: "ApplyExecution::Absent", premises: [], ...extra });
// A producer's named end as the writer logs it (`SiteEnds::log`): `rule: "end"`, the premises it rests on, and the codes the dialect's policy says it answers.
const named = (s, codes, extra = {}) => silent(s, { rule: "end", proof: "local_access.discover_member.placed-elsewhere", premises: [{ kind: "CallEdgePlaces", at: [40, 43] }], codes, ...extra });
const attributed = (s, to, extra = {}) => ({ k: "attributed", ...at(s), code: "SC1001", pass: "DependencySource", to, ...extra });
const kinds = (lines) => proofHits(lines).map((hit) => `${hit.check}/${hit.kind}`);

describe("C2(c): a package finding the dependency-source drop discarded stands again at a project site (ADR 0958)", () => {
  const dropped = (s, extra = {}) => drop(s, { pass: "DependencySource", from: "uncertifiable", premises: [{ kind: "DependencySourceRoot", at: [s, s + 3] }], ...extra });
  test("a drop with no attributed line is a package finding that left the report with no project row", () => {
    assert.deepEqual(kinds([dropped(10)]), ["C2/package-drop-unattributed"]);
  });

  test("an attributed line that lists no site is no attribution", () => {
    assert.deepEqual(kinds([dropped(10), attributed(10, [])]), ["C2/package-drop-unattributed"]);
  });

  test("an attributed line with a project site ends the drop; one at another span does not", () => {
    assert.deepEqual(kinds([dropped(10), attributed(10, [["/p/App.tsx", 100, 110]])]), []);
    assert.deepEqual(kinds([dropped(10), attributed(20, [["/p/App.tsx", 100, 110]])]), ["C2/package-drop-unattributed"]);
  });

  test("a style rule's drop rests on its own named pass and needs no attribution; another pass is not this check's", () => {
    assert.deepEqual(kinds([dropped(10, { pass: "PackageStyleNotProject", code: "SC8003" })]), []);
    assert.deepEqual(kinds([drop(10)]), []);
  });
});

describe("a site has one line per Chrome code (batch 11, slice 4): the checks are asked of each (site, code)", () => {
  const OPTIMISTIC = "OPTIMISTIC_WRITE_BEFORE_LOAD";
  const WRITE = "REACTIVE_WRITE_IN_OWNED_SCOPE";
  const line = (s, code, extra = {}) => site(s, { t: "write", code, ...extra });

  test("the classifier's decisions are about the table's first code: a reporting write decision speaks for the write line, not for SC2009's", () => {
    const writeDecision = decision(10, { t: "write", role: "UntrackedRendering" });
    assert.deepEqual(kinds([line(10, WRITE), writeDecision]), ["C2/reporting-decision-no-row"]);
    assert.deepEqual(kinds([line(10, OPTIMISTIC), writeDecision]), ["C1/no-decision"]);
  });

  test("a line whose ending answers its code is ended; one whose code only another ending answers is a hit again", () => {
    assert.deepEqual(kinds([line(10, OPTIMISTIC, { end: "row" })]), []);
    // `none`: an ending stands at the span but answers another code (`credited: false`), so it ends nothing for this one.
    assert.deepEqual(kinds([line(10, OPTIMISTIC, { by: "rule:no-write-in-pure-computation", credited: false })]), ["C1/no-decision"]);
  });

  test("a named end ends the codes it lists and no other", () => {
    const end = named(10, [WRITE]);
    assert.deepEqual(kinds([line(10, WRITE), decision(10, { t: "write" }), end]), []);
    assert.deepEqual(kinds([line(10, OPTIMISTIC), end]), ["C1/no-decision"]);
    assert.deepEqual(kinds([line(10, OPTIMISTIC), named(10, [WRITE, OPTIMISTIC])]), []);
  });

  test("F1 (the review of #218): a silent line that lists no code ends no code, whatever pass wrote it; a site line with no code is ended by none", () => {
    const write = [line(10, WRITE), decision(10, { t: "write" })];
    const reports = ["C2/reporting-decision-no-row"];
    // The writer omits an empty `codes` list (the end's removed row answers no code of the universe, and the checker files the code's row), and a log from before the field has none.
    assert.deepEqual(kinds([...write, named(10, undefined)]), reports);
    assert.deepEqual(kinds([...write, named(10, [])]), reports);
    // The other passes' lines at the span: a pending proof, a `row:*` stage, a flush proof, SC4001.
    for (const other of [
      silent(10, { rule: "SC5003", proof: "PendingSafe::NotRenderEffect" }),
      silent(10, { rule: "row:reads", proof: "ApplyExecution::Absent" }),
      silent(10, { rule: "SC3001", proof: "FlushSafe::TestRunner" }),
      silent(10, { rule: "SC4001", proof: "ContextProof::OwnedContext" }),
    ]) {
      assert.deepEqual(kinds([...write, other]), reports, other.rule);
    }
    // Not even a line that lists the code, when it is not a named end.
    assert.deepEqual(kinds([...write, silent(10, { rule: "row:reads", codes: [WRITE] })]), reports);
    // A site line with no `code` (a log from before slice 4) has no code for an end to list; its decisions are the table's.
    assert.deepEqual(kinds([site(10, { t: "write", code: undefined }), named(10, [WRITE])]), ["C1/no-decision"]);
    assert.deepEqual(kinds([site(10, { t: "write", code: undefined }), decision(10, { t: "write" })]), reports);
  });

  test("a named end at an alternate span of the site lists its code too", () => {
    const composite = { alt: [[11, 19]] };
    assert.deepEqual(kinds([line(10, OPTIMISTIC, composite), named(11, [OPTIMISTIC], { e: 19 })]), []);
    assert.deepEqual(kinds([line(10, OPTIMISTIC, composite), named(11, [WRITE], { e: 19 })]), ["C1/no-decision"]);
  });

  test("the stats count the lines by code", () => {
    const stats = proofStats([line(10, WRITE, { end: "row" }), line(10, OPTIMISTIC), site(20, { code: "STRICT_READ_UNTRACKED" }), site(30, { code: undefined })]);
    assert.equal(stats.sites, 4);
    assert.deepEqual(stats.siteCodes, { [WRITE]: 1, [OPTIMISTIC]: 1, STRICT_READ_UNTRACKED: 1 });
  });
});

describe("an owner-call site's fail-closed ending is named (batch 11, slice 6; coordinator-approved, owner delegated)", () => {
  const CLEANUP = "CLEANUP_IN_FORBIDDEN_SCOPE";
  const CREATION = "PRIMITIVE_IN_FORBIDDEN_SCOPE";
  const EFFECT = "PRIMITIVE_IN_EFFECT_CALLBACK";
  const call = (s, code, extra = {}) => site(s, { t: "owner-call", source: "primitive", via: "call", code, ...extra });
  // The final report's home row of each code: SC3001's `leaf-operation-unaccounted`, SC6002's `effect-creation-unaccounted`.
  const home = (s, code, extra = {}) =>
    code === EFFECT
      ? verdict(s, { code: "SC6002", rule: "primitive-in-effect-callback", kind: "uncertifiable", ctx: "effect-creation-unaccounted", ...extra })
      : verdict(s, { code: "SC3001", rule: "leaf-owner-forbidden-call", kind: "uncertifiable", ctx: "leaf-operation-unaccounted", ...extra });
  const declined = (s, codes, extra = {}) => ({ k: "declined", ...at(s), rule: "owner-call", proof: "entry-by-reference", codes, ...extra });

  test("a decline that lists the code and the code's home row end the site: no hit", () => {
    assert.deepEqual(kinds([call(10, CLEANUP), home(10, CLEANUP), declined(10, [CLEANUP])]), []);
    assert.deepEqual(kinds([call(10, CREATION), home(10, CREATION), declined(10, [CREATION, CLEANUP])]), []);
  });

  test("the row and the decline may stand at an alternate span of the site", () => {
    const composite = { alt: [[11, 19]] };
    assert.deepEqual(kinds([call(10, CLEANUP, composite), home(11, CLEANUP, { e: 19 }), declined(11, [CLEANUP], { e: 19 })]), []);
    assert.deepEqual(kinds([call(10, CLEANUP, composite), home(10, CLEANUP), declined(11, [CLEANUP], { e: 19 })]), []);
  });

  test("each code of a site is named on its own: a creation's SC3001 and SC6002 codes need each its row and its decline", () => {
    const both = [call(10, CREATION), call(10, EFFECT), home(10, CREATION), home(10, EFFECT)];
    assert.deepEqual(kinds([...both, declined(10, [CREATION, EFFECT])]), []);
    assert.deepEqual(kinds([...both, declined(10, [CREATION])]), ["C1/no-decision"]);
    assert.deepEqual(kinds([...both, declined(10, [EFFECT])]), ["C1/no-decision"]);
  });

  test("a decline alone, or the row alone, is a hit: the ending is named only by both", () => {
    assert.deepEqual(kinds([call(10, CLEANUP), declined(10, [CLEANUP])]), ["C1/no-decision"]);
    assert.deepEqual(kinds([call(10, CLEANUP), home(10, CLEANUP)]), ["C1/no-decision"]);
    // The row of another code's rule is not the code's home row.
    assert.deepEqual(kinds([call(10, EFFECT), home(10, CLEANUP), declined(10, [EFFECT])]), ["C1/no-decision"]);
    // An obligation under another blocker, or a violation, is not the unaccounted row.
    assert.deepEqual(kinds([call(10, CLEANUP), home(10, CLEANUP, { ctx: "function-run-unplaced" }), declined(10, [CLEANUP])]), ["C1/no-decision"]);
    assert.deepEqual(kinds([call(10, CLEANUP), home(10, CLEANUP, { kind: "violation" }), declined(10, [CLEANUP])]), ["C1/no-decision"]);
  });

  test("a decline names the codes it is about, at the site, for the owner-call table only", () => {
    assert.deepEqual(kinds([call(10, CLEANUP), home(10, CLEANUP), declined(10, [CREATION])]), ["C1/no-decision"]);
    assert.deepEqual(kinds([call(10, CLEANUP), home(10, CLEANUP), declined(10, undefined)]), ["C1/no-decision"]);
    assert.deepEqual(kinds([call(10, CLEANUP), home(10, CLEANUP), declined(30, [CLEANUP])]), ["C1/no-decision"]);
    assert.deepEqual(kinds([call(10, CLEANUP), home(10, CLEANUP), declined(10, [CLEANUP], { rule: "pending" })]), ["C1/no-decision"]);
    // Another table's site is no owner-call site: its row and a decline end nothing.
    assert.deepEqual(kinds([site(10, { t: "write", code: "REACTIVE_WRITE_IN_OWNED_SCOPE" }), verdict(10, { code: "SC2001", rule: "no-write-in-pure-computation", kind: "uncertifiable", ctx: "write-unaccounted" }), declined(10, ["REACTIVE_WRITE_IN_OWNED_SCOPE"])]), ["C1/no-decision"]);
  });

  // Gate change (batch 11, B2; coordinator-approved): the projection's ExecutionPaths pass removes a home row a client build never reaches (the arm of an `isServer` ternary), and
  // the drop is the home row's own and carries its typed premise.
  const homeDrop = (s, code, extra = {}) =>
    drop(s, {
      code: code === EFFECT ? "SC6002" : "SC3001",
      from: "uncertifiable",
      ctx: code === EFFECT ? "effect-creation-unaccounted" : "leaf-operation-unaccounted",
      premises: [{ kind: "ClientRuntimeReach", at: [s, s + 3] }],
      ...extra,
    });

  test("gate change: a home row the ExecutionPaths pass dropped with its typed ClientRuntimeReach premise, and a decline, end the site", () => {
    assert.deepEqual(kinds([call(10, EFFECT), homeDrop(10, EFFECT), declined(10, [EFFECT])]), []);
    assert.deepEqual(kinds([call(10, CREATION), homeDrop(10, CREATION), declined(10, [CREATION])]), []);
    // At an alternate span of the site, as the row may be.
    assert.deepEqual(kinds([call(10, EFFECT, { alt: [[11, 19]] }), homeDrop(11, EFFECT, { e: 19 }), declined(10, [EFFECT])]), []);
    // A creation's two codes end each on their own: SC3001's row stands, SC6002's was dropped.
    assert.deepEqual(kinds([call(10, CREATION), call(10, EFFECT), home(10, CREATION), homeDrop(10, EFFECT), declined(10, [CREATION, EFFECT])]), []);
    // The decline is still required, and a drop of another code's home row ends nothing here.
    assert.deepEqual(kinds([call(10, EFFECT), homeDrop(10, EFFECT)]), ["C1/no-decision"]);
    assert.deepEqual(kinds([call(10, EFFECT), homeDrop(10, CREATION), declined(10, [EFFECT])]), ["C1/no-decision"]);
  });

  test("gate change: an untyped drop ends nothing", () => {
    assert.deepEqual(kinds([call(10, EFFECT), homeDrop(10, EFFECT, { premises: [] }), declined(10, [EFFECT])]), ["C1/no-decision"]);
    assert.deepEqual(kinds([call(10, EFFECT), homeDrop(10, EFFECT, { premises: undefined }), declined(10, [EFFECT])]), ["C1/no-decision"]);
    // Another premise kind, or a ClientRuntimeReach that names no span, is no typed premise.
    assert.deepEqual(kinds([call(10, EFFECT), homeDrop(10, EFFECT, { premises: [{ kind: "OwningFindingAtSite", at: [10, 13] }] }), declined(10, [EFFECT])]), ["C1/no-decision"]);
    assert.deepEqual(kinds([call(10, EFFECT), homeDrop(10, EFFECT, { premises: [{ kind: "ClientRuntimeReach" }] }), declined(10, [EFFECT])]), ["C1/no-decision"]);
  });

  test("gate change: a drop of any other pass ends nothing, whatever premise it names, and so does a drop that is not the home row's own", () => {
    for (const pass of ["OwnedByEnabledRule", "RuleDisabled", "HostSuppression", "SupersededByViolation", "SiteUnreached", "RuntimeConfiguration"]) {
      assert.deepEqual(kinds([call(10, EFFECT), homeDrop(10, EFFECT, { pass }), declined(10, [EFFECT])]), ["C1/no-decision"], pass);
    }
    assert.deepEqual(kinds([call(10, EFFECT), homeDrop(10, EFFECT, { code: "SC1001" }), declined(10, [EFFECT])]), ["C1/no-decision"]);
    assert.deepEqual(kinds([call(10, EFFECT), homeDrop(10, EFFECT, { from: "violation" }), declined(10, [EFFECT])]), ["C1/no-decision"]);
    assert.deepEqual(kinds([call(10, EFFECT), homeDrop(10, EFFECT, { ctx: "function-run-unplaced" }), declined(10, [EFFECT])]), ["C1/no-decision"]);
    // A reclass line is no drop.
    assert.deepEqual(kinds([call(10, EFFECT), { ...homeDrop(10, EFFECT), k: "reclass" }, declined(10, [EFFECT])]), ["C1/no-decision"]);
  });
});

describe("C2: a reporting decision with nothing ending the site", () => {
  test("audit candidate S1: the classifier says the read reports, the producer left no row, no finding and no proof", () => {
    assert.deepEqual(kinds([site(10), decision(10)]), ["C2/reporting-decision-no-row"]);
  });

  test("a decision of another kind or another start does not speak for the site", () => {
    assert.deepEqual(kinds([site(10), decision(10, { t: "write" })]), ["C1/no-decision"]);
    assert.deepEqual(kinds([site(10), decision(11)]), ["C1/no-decision"]);
  });

  test("a named silence, a row, a census obligation or the compiler's discard ends the site", () => {
    assert.deepEqual(kinds([site(10), decision(10), named(10, [SITE_CODE.read])]), []);
    assert.deepEqual(kinds([site(10, { end: "row" }), decision(10)]), []);
    assert.deepEqual(kinds([site(10, { end: "census" }), decision(10)]), []);
    assert.deepEqual(kinds([site(10, { end: "discarded" }), decision(10)]), []);
  });

  test("a parenthesized callee's decision starts after the call, and an alternate span of the site finds it", () => {
    const composite = { alt: [[11, 19]], t: "read" };
    assert.deepEqual(kinds([site(10, composite), decision(11)]), ["C2/reporting-decision-no-row"]);
    assert.deepEqual(kinds([site(10, composite), decision(11, { role: "TrackedJsx", silent: true, arm: "callee_roles.tracked-callback" })]), []);
    // A named silence at an alternate span is the site's account too.
    assert.deepEqual(kinds([site(10, composite), decision(11), named(11, [SITE_CODE.read], { e: 19 })]), []);
  });

  test("a props member read with no row is C2 like any other; a named props-static end is its account", () => {
    assert.deepEqual(kinds([site(10, { source: "props", via: "member" }), decision(10)]), ["C2/reporting-decision-no-row"]);
    const end = silent(10, { rule: "end", proof: "local_access.discover_member.props-static", premises: [{ kind: "PropsCallersStatic", at: [3, 4] }], codes: [SITE_CODE.read] });
    assert.deepEqual(kinds([site(10, { source: "props", via: "member" }), decision(10), end]), []);
  });

  test("a store member read with no row is C2", () => {
    assert.deepEqual(kinds([site(10, { source: "store", via: "member" }), decision(10)]), ["C2/reporting-decision-no-row"]);
  });
});

describe("a site that ends in nothing and that no decision reports", () => {
  test("no decision at all is C1", () => {
    assert.deepEqual(kinds([site(10, { t: "store-write", via: "assignment", source: "store" })]), ["C1/no-decision"]);
  });

  test("a tagged, non-default arm that said silent is the account", () => {
    assert.deepEqual(kinds([site(10), decision(10, { role: "TrackedJsx", silent: true, arm: "callee_roles.tracked-region" })]), []);
  });

  test("an untagged or self-declared default silent decision is C3", () => {
    assert.deepEqual(kinds([site(10), decision(10, { role: "DeferredCallback", silent: true, arm: null })]), ["C3/untagged"]);
    assert.deepEqual(kinds([site(10), decision(10, { role: "DeferredCallback", silent: true, arm: "callee_roles.x", default: true })]), ["C3/default-arm"]);
  });
});

describe("C2: a reported row ends in a finding, and a finding in a verdict or a typed drop", () => {
  test("a reported row with no finding and no drop is a lost claim", () => {
    assert.deepEqual(kinds([row(10, { reported: true, role: "UntrackedRendering", why: null })]), ["C2/reported-row-no-finding"]);
  });

  test("a read reported at its origin has its finding where the row says (fa)", () => {
    assert.deepEqual(kinds([row(10, { reported: true, why: null, fa: [40, 43] }), finding(40), verdict(40)]), []);
    assert.deepEqual(kinds([row(10, { reported: true, why: null, fa: [40, 43] })]), ["C2/reported-row-no-finding"]);
  });

  test("a drop alone is a finding the projection itself removed", () => {
    assert.deepEqual(kinds([row(10, { reported: true, why: null }), drop(10)]), []);
  });

  test("a finding must end as a verdict or in a drop", () => {
    const reported = row(10, { reported: true, why: null });
    assert.deepEqual(kinds([reported, finding(10)]), ["C2/finding-lost"]);
    assert.deepEqual(kinds([reported, finding(10), verdict(10)]), []);
    assert.deepEqual(kinds([reported, finding(10), drop(10, { pass: "AbsentProp" })]), []);
    assert.deepEqual(kinds([reported, finding(10), { ...drop(10), k: "reclass", to: "uncertifiable" }]), []);
  });
});

describe("C3: a silent row's role comes from a tagged, non-default arm", () => {
  const silentRow = row(10, { role: "UntrackedCallback", t: "write" });
  const write = (extra) => decision(10, { t: "write", ...extra });

  test("no decision at the site: a producer set the role", () => {
    assert.deepEqual(kinds([silentRow]), ["C3/no-decision"]);
  });

  test("the classifier said another role: an override outside it", () => {
    assert.deepEqual(kinds([silentRow, write({ role: "TrackedJsx" })]), ["C3/override"]);
  });

  test("the role came from an untagged path or a default arm", () => {
    assert.deepEqual(kinds([silentRow, write({ role: "UntrackedCallback", arm: null, silent: true })]), ["C3/untagged"]);
    assert.deepEqual(kinds([silentRow, write({ role: "UntrackedCallback", arm: "write_roles.x", default: true, silent: true })]), ["C3/default-arm"]);
  });

  test("a tagged, non-default arm explains it", () => {
    assert.deepEqual(kinds([silentRow, write({ role: "UntrackedCallback", arm: "write_roles.x", silent: true })]), []);
  });

  test("a row silent for a flag, not for its role, is not C3", () => {
    assert.deepEqual(kinds([row(10, { why: "owned-write-on", role: "UntrackedRendering", t: "write" })]), []);
  });
});

describe("a producer's named end rests on a premise that names code", () => {
  const end = (s, premises, extra = {}) => silent(s, { rule: "end", proof: "local_access.discover_member.placed-elsewhere", premises, codes: [SITE_CODE.read], ...extra });

  test("a fact kind with the span of the code it is about is the site's account", () => {
    const premises = [{ kind: "CallEdgePlaces", at: [40, 43] }, { kind: "CallEdgePlaces", at: [4, 5], in: "/p/B.tsx" }];
    assert.deepEqual(kinds([site(10, { source: "store", via: "member" }), decision(10), end(10, premises)]), []);
  });

  test("an end with no premise, or a premise with no span, is a label: C3/label-only-end", () => {
    assert.deepEqual(kinds([site(10), decision(10), end(10, [])]), ["C3/label-only-end"]);
    assert.deepEqual(kinds([site(10), decision(10), end(10, [{ kind: "CarrierPlacesRead", at: null }])]), ["C3/label-only-end"]);
  });

  test("the typed proofs of the accounting passes are not named ends: they are no label-only end, and they end no code of the universe", () => {
    assert.deepEqual(kinds([site(10), decision(10), silent(10, { rule: "SC4001", proof: "ContextProof::OwnedContext" })]), ["C2/reporting-decision-no-row"]);
  });

  test("a label-only end is never an accepted gap, whatever it says", () => {
    const hit = proofHits([site(10), decision(10), end(10, [])])[0];
    const key = hitKey("fx", "App.tsx", hit);
    const gaps = { [key]: { check: "C3", why: "the producer returns here and the carrier call places the function" } };
    assert.deepEqual(gapProblems(new Map([[key, hit]]), gaps).map((problem) => problem.kind), ["invalid"]);
  });

  describe("a setter whose write guard throws first (ADR 0932)", () => {
    const guard = (s, extra = {}) => end(s, [{ kind: "SetterGuardThrowsFirst", at: [40, 52] }], { proof: "local_access.discover_call_as.setter-guard-throws-first", ...extra });
    const violation = (extra = {}) => verdict(40, { e: 52, code: "SC2001", kind: "violation", ...extra });

    test("the setter call's SC2001 violation in the final list confirms the premise", () => {
      assert.deepEqual(kinds([site(10), decision(10), guard(10), violation()]), []);
    });

    test("no violation at the span, one of another rule, or only an uncertifiable one leaves the premise unconfirmed", () => {
      assert.deepEqual(kinds([site(10), decision(10), guard(10)]), ["C3/premise-unconfirmed"]);
      assert.deepEqual(kinds([site(10), decision(10), guard(10), violation({ code: "SC1001" })]), ["C3/premise-unconfirmed"]);
      assert.deepEqual(kinds([site(10), decision(10), guard(10), violation({ kind: "uncertifiable" })]), ["C3/premise-unconfirmed"]);
      assert.deepEqual(kinds([site(10), decision(10), guard(10), violation({ s: 41 })]), ["C3/premise-unconfirmed"]);
    });

    test("a premise in another file cannot be confirmed by this file's verdicts", () => {
      const elsewhere = end(10, [{ kind: "SetterGuardThrowsFirst", at: [40, 52], in: "/p/B.tsx" }]);
      assert.deepEqual(kinds([site(10), decision(10), elsewhere, violation()]), ["C3/premise-unconfirmed"]);
    });

    test("an unconfirmed premise is never an accepted gap", () => {
      const hit = proofHits([site(10), decision(10), guard(10)])[0];
      const key = hitKey("fx", "App.tsx", hit);
      const gaps = { [key]: { check: "C3", why: "the guard throws first" } };
      assert.deepEqual(gapProblems(new Map([[key, hit]]), gaps).map((problem) => problem.kind), ["invalid"]);
    });
  });
});

describe("the gap file", () => {
  const hit = (check, kind) => ({ check, kind, s: 1, e: 2 });
  const byKey = (...hits) => new Map(hits.map((h) => [hitKey("fx", "App.tsx", h), { ...h, fixture: "fx" }]));
  const key = (check, kind) => hitKey("fx", "App.tsx", hit(check, kind));
  const sound = "the tracked region is a compiler fact: the read runs inside a computation";

  test("a hit no entry accepts is new, an entry no hit matches is stale", () => {
    assert.deepEqual(
      gapProblems(byKey(hit("C3", "untagged")), {}).map((p) => p.kind),
      ["new"]
    );
    assert.deepEqual(
      gapProblems(new Map(), { [key("C3", "untagged")]: { check: "C3", why: sound } }).map((p) => p.kind),
      ["stale"]
    );
    assert.deepEqual(gapProblems(byKey(hit("C3", "untagged")), { [key("C3", "untagged")]: { check: "C3", why: sound } }), []);
  });

  test("a C2 hit is never an accepted gap, only a candidate under review", () => {
    const hits = byKey(hit("C2", "reporting-decision-no-row"));
    assert.deepEqual(
      gapProblems(hits, { [key("C2", "reporting-decision-no-row")]: { check: "C2", why: sound } }).map((p) => p.kind),
      ["invalid"]
    );
    assert.deepEqual(gapProblems(hits, {}).map((p) => p.kind), ["new"]);
    assert.deepEqual(
      gapProblems(hits, {}, { [key("C2", "reporting-decision-no-row")]: { owner: "fix 5" } }).map((p) => p.kind),
      ["candidate"]
    );
    assert.deepEqual(gapProblems(new Map(), {}, { [key("C2", "reporting-decision-no-row")]: { owner: "fix 5" } }).map((p) => p.kind), ["stale"]);
  });

  test("a C1 or C3 hit under review is a candidate too, never an accepted gap", () => {
    const hits = byKey(hit("C3", "no-decision"));
    const candidates = { [key("C3", "no-decision")]: { owner: "fix 6" } };
    assert.deepEqual(gapProblems(hits, {}, candidates).map((p) => p.kind), ["candidate"]);
    assert.deepEqual(gapProblems(new Map(), {}, candidates).map((p) => p.kind), ["stale"]);
  });

  test("pruning removes the candidate entries no hit matches and nothing else", () => {
    const hits = byKey(hit("C2", "reporting-decision-no-row"));
    const candidates = {
      [key("C2", "reporting-decision-no-row")]: { owner: "fix 6" },
      [key("C2", "other")]: { owner: "#197" },
    };
    const { kept, removed } = pruneCandidates(candidates, hits);
    assert.deepEqual(Object.keys(kept), [key("C2", "reporting-decision-no-row")]);
    assert.deepEqual(removed, [key("C2", "other")]);
  });

  test("a reason that states no fact is invalid", () => {
    for (const why of ["", "known", "Harmless.", "TODO", "UNREVIEWED", "short", undefined]) {
      const problems = gapProblems(byKey(hit("C3", "untagged")), { [key("C3", "untagged")]: { check: "C3", why } });
      assert.deepEqual(problems.map((p) => p.kind), ["invalid"], JSON.stringify(why));
    }
    assert.ok(EMPTY_REASONS.test("known"));
    assert.ok(!EMPTY_REASONS.test(sound));
  });
});

describe("the log reader", () => {
  test("a line names its file through the path line of its process", () => {
    const paths = new Map();
    const counter = { malformed: 0 };
    assert.equal(resolvedLine({ k: "path", q: 7, p: 0, f: "/p/A.tsx", g: "fx" }, paths, counter), null);
    assert.deepEqual(resolvedLine({ k: "row", q: 7, p: 0, s: 1, e: 2 }, paths, counter), { k: "row", s: 1, e: 2, f: "/p/A.tsx", g: "fx" });
    assert.equal(resolvedLine({ k: "row", q: 7, p: 9, s: 1, e: 2 }, paths, counter), null);
    assert.equal(counter.malformed, 1);
  });

  test("a log file is read line by line, a malformed line counted", () => {
    const directory = mkdtempSync(join(tmpdir(), "proof-log-test-"));
    try {
      const file = join(directory, "a.jsonl");
      writeFileSync(file, [JSON.stringify({ k: "path", q: 1, p: 0, f: "/p/A.tsx" }), JSON.stringify({ k: "site", q: 1, p: 0, s: 1, e: 2, t: "read", end: "none" }), "{not json"].join("\n"));
      const counter = { malformed: 0 };
      const lines = [...proofLogLines(file, counter)];
      assert.equal(lines.length, 1);
      assert.equal(lines[0].f, "/p/A.tsx");
      assert.equal(counter.malformed, 1);
      assert.deepEqual(proofStats(lines).siteEnds, { none: 1 });
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });
});

describe("the fixture projects", () => {
  test("are the projects `make coverage` holds a snapshot for", () => {
    const projects = fixtureProjects(ROOT).map((project) => `${project.id.replace("/", "__")}.json`);
    const snapshots = readdirSync(join(ROOT, "fixtures", "findings-snapshots")).filter((name) => name.endsWith(".json"));
    assert.deepEqual([...projects].sort(), [...snapshots].sort());
  });
});
