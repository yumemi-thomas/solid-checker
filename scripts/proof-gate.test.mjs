import assert from "node:assert/strict";
import { mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, test } from "vitest";

import { ROOT } from "./proof-gate.mjs";
import { fixtureProjects } from "./lib/fixture-projects.mjs";
import { EMPTY_REASONS, gapProblems, hitKey, proofHits, proofLogLines, proofStats, pruneCandidates, resolvedLine } from "./lib/proof-log.mjs";

const F = "/p/App.tsx";
const at = (s, e = s + 3) => ({ f: F, s, e });
const decision = (s, extra = {}) => ({ k: "decision", ...at(s), t: "read", role: "UntrackedRendering", arm: "callee_roles.inside-component", arms: [], default: false, silent: false, ...extra });
const site = (s, extra = {}) => ({ k: "site", ...at(s), t: "read", source: "accessor", via: "call", end: "none", detail: "", ...extra });
const row = (s, extra = {}) => ({ k: "row", ...at(s), t: "read", role: "TrackedJsx", reported: false, why: "role-does-not-report", ...extra });
const finding = (s, extra = {}) => ({ k: "finding", ...at(s), code: "SC1001", kind: "violation", ctx: "", ...extra });
const verdict = (s, extra = {}) => ({ ...finding(s, extra), k: "verdict" });
const drop = (s, extra = {}) => ({ k: "drop", ...at(s), code: "SC1001", from: "violation", ctx: "", pass: "ExecutionPaths", premises: [], ...extra });
const silent = (s, extra = {}) => ({ k: "silent", ...at(s), rule: "row:reads", proof: "ApplyExecution::Absent", premises: [], ...extra });
const kinds = (lines) => proofHits(lines).map((hit) => `${hit.check}/${hit.kind}`);

describe("C2: a reporting decision with nothing ending the site", () => {
  test("audit candidate S1: the classifier says the read reports, the producer left no row, no finding and no proof", () => {
    assert.deepEqual(kinds([site(10), decision(10)]), ["C2/reporting-decision-no-row"]);
  });

  test("a decision of another kind or another start does not speak for the site", () => {
    assert.deepEqual(kinds([site(10), decision(10, { t: "write" })]), ["C1/no-decision"]);
    assert.deepEqual(kinds([site(10), decision(11)]), ["C1/no-decision"]);
  });

  test("a named silence, a row, a census obligation or the compiler's discard ends the site", () => {
    assert.deepEqual(kinds([site(10), decision(10), silent(10)]), []);
    assert.deepEqual(kinds([site(10, { end: "row" }), decision(10)]), []);
    assert.deepEqual(kinds([site(10, { end: "census" }), decision(10)]), []);
    assert.deepEqual(kinds([site(10, { end: "discarded" }), decision(10)]), []);
  });

  test("a parenthesized callee's decision starts after the call, and an alternate span of the site finds it", () => {
    const composite = { alt: [[11, 19]], t: "read" };
    assert.deepEqual(kinds([site(10, composite), decision(11)]), ["C2/reporting-decision-no-row"]);
    assert.deepEqual(kinds([site(10, composite), decision(11, { role: "TrackedJsx", silent: true, arm: "callee_roles.tracked-callback" })]), []);
    // A named silence at an alternate span is the site's account too.
    assert.deepEqual(kinds([site(10, composite), decision(11), silent(11, { e: 19 })]), []);
  });

  test("a props member read with no row is C2 like any other; a named props-static end is its account", () => {
    assert.deepEqual(kinds([site(10, { source: "props", via: "member" }), decision(10)]), ["C2/reporting-decision-no-row"]);
    const end = silent(10, { rule: "end", proof: "local_access.discover_member.props-static", premises: [{ kind: "PropsCallersStatic", at: [3, 4] }] });
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
  const end = (s, premises, extra = {}) => silent(s, { rule: "end", proof: "local_access.discover_member.placed-elsewhere", premises, ...extra });

  test("a fact kind with the span of the code it is about is the site's account", () => {
    const premises = [{ kind: "CallEdgePlaces", at: [40, 43] }, { kind: "CallEdgePlaces", at: [4, 5], in: "/p/B.tsx" }];
    assert.deepEqual(kinds([site(10, { source: "store", via: "member" }), decision(10), end(10, premises)]), []);
  });

  test("an end with no premise, or a premise with no span, is a label: C3/label-only-end", () => {
    assert.deepEqual(kinds([site(10), decision(10), end(10, [])]), ["C3/label-only-end"]);
    assert.deepEqual(kinds([site(10), decision(10), end(10, [{ kind: "CarrierPlacesRead", at: null }])]), ["C3/label-only-end"]);
  });

  test("the typed proofs of the accounting passes are not named ends", () => {
    assert.deepEqual(kinds([site(10), decision(10), silent(10, { rule: "SC4001", proof: "ContextProof::OwnedContext" })]), []);
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
