import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { classify } from "./classify.mjs";
import { judgeUnexecutedInChrome } from "./pipeline.mjs";
import { closedGrammar } from "./model.mjs";
import { LEXICALLY_DEAD, STATUS, lexicallyDeadHits, siteBasis, summarizeUnexecuted, unexecutedCandidate } from "./unexecuted.mjs";
import { dedupe } from "./dedupe.mjs";

// Two containers programs, as the generator builds them (and-false below an onCleanup handle called now; a component nobody renders
// calling a helper closure declared beside it). Their text is printed by buildCase, so the ledger and the lines are the real ones.
const signal = { k: "signal", name: "w1", setter: "setW1", init: 0, role: "probe", scope: "component" };
const fn = (body, over = {}) => ({ k: "fn", form: "arrow", async: false, params: [], body, ...over });
const write = { k: "write", sig: "w1", value: 17, site: true, witness: { selector: "[data-probe=\"w1\"]", text: "17" } };
const andFalse = {
  k: "program", version: 1, seed: 1, helpers: [], moduleSignals: [],
  components: [{ k: "component", name: "C1", signals: [signal], body: [], view: [{ k: "probe", sig: "w1" }, { k: "child", id: "v1", e: { k: "iife", fn: fn([
    { k: "decl", name: "h1", e: { k: "own", op: "cleanup", handle: true, fn: fn([{ k: "contain", kind: "and-false", id: 1, async: false, use: null, body: [{ k: "expr", e: { ...write, via: "signal" } }] }]) } },
    { k: "expr", e: { k: "call", callee: { k: "ref", name: "h1" }, args: [] } },
    { k: "return", e: { k: "num", n: 1 } },
  ]) } }] }],
};
const compNever = (kind = "comp-never") => ({
  k: "program", version: 1, seed: 2, helpers: [], moduleSignals: [],
  components: [{ k: "component", name: "C1", signals: [signal], view: [{ k: "probe", sig: "w1" }], body: [
    { k: "decl", name: "f1", e: fn([{ k: "return", e: write }]) },
    { k: "contain", kind, id: 1, async: false, use: null, body: [{ k: "expr", e: { k: "call", callee: { k: "ref", name: "f1" }, args: [] } }, { k: "return", e: { k: "jsx" } }] },
  ] }],
});
// A closed program (no container): a helper closure declared and never called.
const strictClosed = {
  k: "program", version: 1, seed: 3, helpers: [], moduleSignals: [],
  components: [{ k: "component", name: "C1", signals: [signal], view: [{ k: "probe", sig: "w1" }], body: [{ k: "decl", name: "f1", e: fn([{ k: "return", e: write }]) }] }],
};
const caseOf = (program) => ({ program, built: buildCase(program, { id: `t-${program.seed}` }) });
const violationAt = (built, rule = "no-write-in-pure-computation") => { const [site] = built.sites; return { rule, id: "SC2001", kind: "violation", line: site.line, endLine: site.line }; };
/** Hits: every entry and glue counter ran once; the payload sites never ran. */
const hitsOf = (built) => Object.fromEntries(built.ledger.filter((row) => row.kind !== "site").map((row) => [row.hit, 1]));
const runtime = (built, hits, over = {}) => ({ failure: null, diagnostics: [], errors: [], witnesses: built.sites.map((site) => ({ siteId: site.id, reached: null })), hits, quiescent: true, ...over });
const judge = (c, runtimeRecord, extra = {}) => classify({
  sites: c.built.sites, glue: c.built.glue, checker: { ok: true, findings: [violationAt(c.built)] }, runtime: runtimeRecord, plan: c.built.plan,
  evidence: { ledger: c.built.ledger, closed: closedGrammar(c.program), neutral: true, lexicallyDead: lexicallyDeadHits(c.program, c.built.ledger), ...extra },
});

describe("lexicallyDeadHits", () => {
  it("names the ledger rows below if (false), after return and false &&, and no others", () => {
    const c = caseOf(andFalse);
    const dead = lexicallyDeadHits(c.program, c.built.ledger);
    const [site] = c.built.sites;
    expect(dead.has(site.hit)).toBe(true);
    expect(dead.size).toBeGreaterThan(0);
    const never = caseOf(compNever());
    expect(lexicallyDeadHits(never.program, never.built.ledger).size).toBe(0);
    expect([...LEXICALLY_DEAD].sort()).toEqual(["and-false", "dead-after-return", "dead-if"]);
  });
});

describe("siteBasis and unexecutedCandidate", () => {
  it("reads why the ledger says a site did not run", () => {
    const dead = caseOf(andFalse);
    const [site] = dead.built.sites;
    const hits = hitsOf(dead.built);
    expect(siteBasis({ site, ledger: dead.built.ledger, hits, lexicallyDead: lexicallyDeadHits(dead.program, dead.built.ledger) })).toEqual({ placed: true, hostRan: true, basis: "dead-code" });
    const never = caseOf(compNever());
    const [helperSite] = never.built.sites;
    // f1's entry (the site's nearest function) and the container's entry never ran; the component's did.
    const component = never.built.ledger.find((row) => row.node === "component");
    expect(siteBasis({ site: helperSite, ledger: never.built.ledger, hits: { [component.hit]: 1 } })).toMatchObject({ placed: true, hostRan: true, basis: "function-not-entered" });
    // entered nearest function, skipped site: the ledger cannot say why
    expect(siteBasis({ site: helperSite, ledger: never.built.ledger, hits: hitsOf(never.built) })).toMatchObject({ basis: "skipped-in-entered-function" });
    // no component entered: the host never ran
    expect(siteBasis({ site: helperSite, ledger: never.built.ledger, hits: {} })).toMatchObject({ placed: true, hostRan: false });
    // the ledger cannot place it
    expect(siteBasis({ site: { id: "sX", hit: 9999 }, ledger: never.built.ledger, hits: {} })).toMatchObject({ placed: false });
    expect(siteBasis({ site: helperSite, ledger: never.built.ledger, hits: null })).toMatchObject({ placed: false });
  });

  it("is a candidate only with every site placed, at zero, the host entered and the page quiescent", () => {
    const c = caseOf(andFalse);
    const hits = hitsOf(c.built);
    const lexicallyDead = lexicallyDeadHits(c.program, c.built.ledger);
    const ok = unexecutedCandidate({ sites: c.built.sites, ledger: c.built.ledger, hits, quiescent: true, lexicallyDead });
    expect(ok).toEqual({ ok: true, bases: ["dead-code"] });
    for (const [over, why] of [[{ quiescent: false }, /moved late/], [{ quiescent: null }, /no quiescence/], [{ hits: null }, /no hit readout/], [{ sites: [] }, /no payload site/], [{ hits: { ...hits, [c.built.sites[0].hit]: 1 } }, /ran/], [{ hits: {} }, /no function enclosing/]]) {
      const answer = unexecutedCandidate({ sites: c.built.sites, ledger: c.built.ledger, hits, quiescent: true, lexicallyDead, ...over });
      expect(answer.ok).toBe(false);
      expect(answer.why).toMatch(why);
    }
  });
});

describe("classify: candidate-false-positive: unexecuted", () => {
  it("a violation at a payload site the ledger shows never ran is the typed candidate, not inconclusive and not a false positive", () => {
    for (const c of [caseOf(andFalse), caseOf(compNever())]) {
      const result = judge(c, runtime(c.built, c.program.seed === 1 ? hitsOf(c.built) : { [c.built.ledger.find((row) => row.node === "component").hit]: 1 }));
      expect(result.verdict).toBe("candidate-unexecuted");
      expect(result.items[0]).toMatchObject({ kind: "candidate-unexecuted", status: STATUS, siteIds: ["s1"] });
      expect(result.items[0].kind).not.toBe("candidate-false-positive");
    }
    expect(STATUS).toBe("candidate-false-positive: unexecuted");
  });

  it("names the basis: dead code, a container never entered, a function never entered", () => {
    const dead = caseOf(andFalse);
    expect(judge(dead, runtime(dead.built, hitsOf(dead.built))).items[0].bases).toEqual(["dead-code"]);
    const never = caseOf(compNever());
    const hits = { [never.built.ledger.find((row) => row.node === "component").hit]: 1 };
    expect(judge(never, runtime(never.built, hits)).items[0].bases).toEqual(["function-not-entered"]);
  });

  it("stays inconclusive: never-ran for everything the ledger cannot prove", () => {
    const c = caseOf(andFalse);
    const hits = hitsOf(c.built);
    const stays = (result) => expect(result.items[0]).toMatchObject({ kind: "inconclusive", why: "never-ran" });
    stays(judge(c, runtime(c.built, hits, { quiescent: false })));
    stays(judge(c, runtime(c.built, hits, { quiescent: null })));
    stays(judge(c, runtime(c.built, {})));                       // the host never ran: a harness fault
    stays(judge(c, runtime(c.built, hits), { ledger: [] }));     // the ledger cannot place the site
    // a plumbing-line finding is judged by the glue calls and is never a candidate
    const glue = c.built.glue[0];
    const plumbing = classify({ sites: [], glue: c.built.glue, checker: { ok: true, findings: [{ rule: "strict-read-untracked", id: "SC3001", kind: "violation", line: glue.line, endLine: glue.line }] },
      runtime: runtime(c.built, Object.fromEntries(c.built.ledger.filter((row) => row.kind === "entry").map((row) => [row.hit, 1]))), plan: c.built.plan, evidence: { ledger: c.built.ledger, lexicallyDead: lexicallyDeadHits(c.program, c.built.ledger) } });
    expect(plumbing.items[0]).toMatchObject({ kind: "inconclusive", why: "never-ran" });
  });

  it("a site that ran, a disturbed run and a runtime diagnostic at the finding keep their old verdicts", () => {
    const c = caseOf(andFalse);
    const [site] = c.built.sites;
    const ran = judge(c, runtime(c.built, { ...hitsOf(c.built), [site.hit]: 1 }, { witnesses: [{ siteId: site.id, reached: true }] }));
    expect(ran.verdict).toBe("candidate-false-positive");
    expect(judge(c, runtime(c.built, hitsOf(c.built), { errors: [{ kind: "window-error", message: "boom", positions: [] }] })).items[0]).toMatchObject({ kind: "inconclusive", why: "disturbed" });
    const diagnostic = { code: "REACTIVE_WRITE_IN_OWNED_SCOPE", message: "x", positions: [{ file: "App.tsx", line: site.line }] };
    expect(judge(c, runtime(c.built, hitsOf(c.built), { diagnostics: [diagnostic] })).verdict).toBe("agree");
  });

  it("does not outrank a recall gap, so only rows that were inconclusive: never-ran move to it", () => {
    const c = caseOf(andFalse);
    const [site] = c.built.sites;
    const result = classify({
      sites: c.built.sites, glue: c.built.glue, plan: c.built.plan, runtime: runtime(c.built, hitsOf(c.built), { diagnostics: [{ code: "NO_OWNER_CLEANUP", message: "x", positions: [{ file: "App.tsx", line: 1 }] }] }),
      checker: { ok: true, findings: [violationAt(c.built), { rule: "missing-owner", id: "SC4001", kind: "uncertifiable", line: 1, endLine: 1 }] },
      evidence: { ledger: c.built.ledger, lexicallyDead: lexicallyDeadHits(c.program, c.built.ledger) },
    });
    expect(result.items.map((item) => item.kind).sort()).toEqual(["candidate-unexecuted", "recall-gap"]);
    expect(result.verdict).toBe("recall-gap");
    expect(site).toBeTruthy();
  });
});

describe("Chrome's judgement of an unexecuted candidate", () => {
  const prepare = (c, hits, over = {}) => {
    const first = judge(c, runtime(c.built, hits));
    const record = { row: {}, runtime: runtime(c.built, hits, over.runtime), checker: { ok: true, findings: [violationAt(c.built)], perturbed: [], ...over.checker } };
    return { item: first.items[0], record, value: { ...c, control: { matched: true } } };
  };
  const confirm = (c, hits, over = {}, options = {}) => {
    const { item, record, value } = prepare(c, hits, over);
    return judgeUnexecutedInChrome(value, item, record, { controlMatched: true, ...options });
  };
  const neverHits = (c) => ({ [c.built.ledger.find((row) => row.node === "component").hit]: 1 });

  it("confirms when Chrome shows the same and every condition holds: dead code needs no closed grammar, a container program the container-aware one", () => {
    const dead = caseOf(andFalse);
    expect(closedGrammar(dead.program).closed).toBe(false); // the strict grammar refuses every container
    expect(confirm(dead, hitsOf(dead.built))).toMatchObject({ status: "confirmed-unexecuted", why: "confirmed" });
    const never = caseOf(compNever());
    expect(confirm(never, neverHits(never))).toMatchObject({ status: "confirmed-unexecuted" });
  });

  it("--dead-site's strict mode asks the old conditions: no container grammar, no dead-code waiver", () => {
    const never = caseOf(compNever());
    expect(confirm(never, neverHits(never), {}, { strict: true })).toMatchObject({ status: "chrome-inconclusive", why: "conditions-unmet", unmet: ["closed-grammar"] });
    // dead code: the default mode waives the grammar and the no-throw reading for it, the strict mode does not
    const dead = caseOf(andFalse);
    expect(confirm(dead, hitsOf(dead.built), {}, { strict: true })).toMatchObject({ status: "chrome-inconclusive", unmet: ["closed-grammar", "not-a-throw"] });
    // a program inside the strict grammar is confirmed by either mode (the strict conditions are a subset of the default ones)
    const closedProgram = caseOf(strictClosed);
    expect(closedGrammar(closedProgram.program).closed).toBe(true);
    const hits = { [closedProgram.built.ledger.find((row) => row.node === "component").hit]: 1 };
    for (const strict of [true, false]) expect(confirm(closedProgram, hits, {}, { strict })).toMatchObject({ status: "confirmed-unexecuted" });
  });

  it("refuses a comp-prop container: the caller may supply the prop", () => {
    const never = caseOf(compNever("comp-prop"));
    expect(closedGrammar(never.program, { containers: true }).closed).toBe(false);
    expect(confirm(never, neverHits(never))).toMatchObject({ status: "chrome-inconclusive", why: "conditions-unmet", unmet: ["closed-grammar"] });
  });

  it("types every other outcome", () => {
    const dead = caseOf(andFalse);
    const hits = hitsOf(dead.built);
    const [site] = dead.built.sites;
    expect(confirm(dead, hits, { runtime: { hits: { ...hits, [site.hit]: 1 }, witnesses: [{ siteId: site.id, reached: true }] } })).toMatchObject({ status: "refuted-by-chrome", why: "chrome-ran" });
    const diagnostic = { code: "REACTIVE_WRITE_IN_OWNED_SCOPE", message: "x", positions: [{ file: "App.tsx", line: site.line }] };
    expect(confirm(dead, hits, { runtime: { diagnostics: [diagnostic] } })).toMatchObject({ status: "refuted-by-chrome", why: "chrome-diagnostic" });
    // a diagnostic of the rule's codes elsewhere in the program: the claim is about the whole program
    const elsewhere = { code: "REACTIVE_WRITE_IN_OWNED_SCOPE", message: "x", positions: [{ file: "App.tsx", line: 1 }] };
    expect(confirm(dead, hits, { runtime: { diagnostics: [elsewhere] } })).toMatchObject({ status: "chrome-inconclusive", why: "rule-code-reported" });
    expect(confirm(dead, hits, { checker: { ok: false, error: "boom" } })).toMatchObject({ status: "chrome-inconclusive", why: "chrome-failed" });
    expect(confirm(dead, hits, { checker: { perturbed: ["new on the instrumented text: x|violation|9"] } })).toMatchObject({ status: "chrome-inconclusive", why: "instrumentation-perturbed-checker" });
    expect(confirm(dead, hits, { checker: { findings: [] } })).toMatchObject({ status: "chrome-inconclusive", why: "chrome-checker-differs" });
    expect(confirm(dead, hits, { runtime: { quiescent: false } })).toMatchObject({ status: "chrome-inconclusive", why: "never-ran" });
    expect(confirm(dead, hits, { runtime: { errors: [{ kind: "page-error", message: "boom", positions: [] }] } })).toMatchObject({ status: "chrome-inconclusive", why: "disturbed" });
    // the fast path's control: without a matching clean twin the instrumented run is not shown neutral
    const { item, record, value } = prepare(dead, hits);
    expect(judgeUnexecutedInChrome({ ...value, control: { matched: false } }, item, record, { controlMatched: false })).toMatchObject({ status: "chrome-inconclusive", why: "conditions-unmet", unmet: ["neutral"] });
    expect(judgeUnexecutedInChrome(value, item, record, { controlMatched: null })).toMatchObject({ unmet: ["neutral"] });
  });

  it("a skipped site in an entered function is not provable unless the text skips it", () => {
    const never = caseOf(compNever());
    // the helper's own entry ran, its site did not, and nothing in the text explains it: a swallowed throw would look the same
    expect(confirm(never, hitsOf(never.built))).toMatchObject({ status: "chrome-inconclusive", why: "conditions-unmet", unmet: ["not-a-throw"] });
  });
});

describe("summarizeUnexecuted", () => {
  it("counts by status and basis and groups by signature", () => {
    const rows = [
      { id: "a", signature: "s1", status: "confirmed-unexecuted", why: "confirmed", bases: ["dead-code"], size: 20 },
      { id: "b", signature: "s1", status: "chrome-inconclusive", why: "conditions-unmet", bases: ["dead-code", "dead-code"], size: 30 },
      { id: "c", signature: "s2", status: "unconfirmed", why: "no-chrome", bases: ["function-not-entered"], size: 25 },
    ];
    const report = summarizeUnexecuted(rows, dedupe);
    expect(report).toMatchObject({ candidates: 3, programs: 3, confirmed: 1, byStatus: { "confirmed-unexecuted": 1, "chrome-inconclusive:conditions-unmet": 1, unconfirmed: 1 }, byBasis: { "dead-code": 2, "function-not-entered": 1 } });
    expect(report.groups.map((group) => [group.signature, group.count])).toEqual([["s1", 2], ["s2", 1]]);
    expect(summarizeUnexecuted([], dedupe)).toMatchObject({ candidates: 0, confirmed: 0, groups: [] });
  });
});
