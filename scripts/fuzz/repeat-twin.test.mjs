import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { runtimeEvents } from "./classify.mjs";
import { writeCase } from "./emit.mjs";
import { DEFAULT_FAST_INSTALL, runFast } from "./fast.mjs";
import { cleanPositions } from "./hits.mjs";
import { generateShape, judge, otherRegions, recallShapes, settleRepeats, shapeId, siteOf, twinRequests } from "./matrix.mjs";
import { getAt } from "./model.mjs";
import { repeatTwins, twinHolds } from "./repeat-twin.mjs";
import { STACKED_PAIRS, isRepeat, stackedSplit } from "./stacked.mjs";

// ADR 0953, option (c) (owner's ruling): a repeat claim is the agreement it is only where its twin holds. The twin is the program with the first read removed,
// judged at the remaining read; where it fails the claim stays a stacked claim, so a twin never turns a false clean or a false positive into agreement.

const claim = (finding, extra = {}) => ({ finding, verdict: "stacked", first: "UNTRACKED_READ_AFTER_AWAIT", why: "repeat", ...extra });
const RULE = "reactive-read-after-await";

describe("stackedSplit with twins", () => {
  const one = claim(`${RULE}@8`);
  it("a repeat claim whose twin holds is neither stacked nor wrong", () => {
    expect(stackedSplit([one], undefined, { twins: { [one.finding]: { holds: true } } })).toEqual({ wrong: [], stacked: [], twinned: [one] });
  });
  it("a repeat claim with no twin, a failed twin or a twin of another claim is split by its pair as before", () => {
    for (const twins of [undefined, {}, { [one.finding]: { holds: false, why: "twin false-clean" } }, { [`${RULE}@9`]: { holds: true } }]) {
      expect(stackedSplit([one], undefined, { twins })).toEqual({ wrong: [], stacked: [one], twinned: [] });
    }
  });
  it("a twin settles no claim of any other kind: a first-throw stack, an unproved pair, a disturbed run's repeat, a false positive", () => {
    const rows = [
      { finding: "missing-owner@5", verdict: "stacked", first: "PRIMITIVE_IN_EFFECT_CALLBACK", why: "disturbed" },
      { finding: "strict-read-untracked@6", verdict: "stacked", first: "STRICT_READ_UNTRACKED", why: "repeat" },
      claim(`${RULE}@7`, { why: "disturbed-repeat" }),
      { finding: "flush-in-action@9", verdict: "false-positive" },
    ];
    const twins = Object.fromEntries(rows.map((row) => [row.finding, { holds: true }]));
    const split = stackedSplit(rows, undefined, { twins });
    expect(split.twinned).toEqual([]);
    expect(split.stacked.map((row) => row.finding)).toEqual(["missing-owner@5", `${RULE}@7`]);
    expect(split.wrong.map((row) => row.finding)).toEqual(["strict-read-untracked@6", "flush-in-action@9"]);
    expect(rows.map((row) => isRepeat(row))).toEqual([false, false, false, false]);
  });
  it("the pair table marks exactly the two repeat pairs", () => {
    expect(Object.entries(STACKED_PAIRS).filter(([, pair]) => pair.kind === "repeat").map(([name]) => name)).toEqual(["UNTRACKED_READ_AFTER_AWAIT|reactive-read-after-await", "STORE_WRITE_OUTSIDE_SETTER|no-store-mutation-outside-setter"]);
  });
});

// The census cell's program: one source read before and after the helper's own await, SC1002 claimed at both reads, Solid reports the first.
const cell = { code: "UNTRACKED_READ_AFTER_AWAIT", context: "aw-helper-late" };
const shape = recallShapes(cell.code, cell.context).find((one) => shapeId(cell, one) === "mx-untracked-read-after-await-aw-helper-late-x-direct-none");
const { program, built } = generateShape({ seed: "1" }, cell, shape, 0);
const [first, second] = built.sites;
const column = (text, line) => text.split("\n")[line - 1].search(/\S/) + 1;
/** A violation of `rule` starting at a recorded frame's own position (the checker's findings sit where the runtime's frames do), or `at` columns from the line's first character. */
const violation = (text, line, rule = RULE, offset = 0) => ({ id: "SC1002", rule, kind: "violation", line, column: column(text, line) + offset, endLine: line, file: "App.tsx" });
const label = `${RULE}@${second.line}`;
const asked = (extra = {}) => claim(label, { firstAt: [{ line: first.line, column: column(built.text, first.line) }], ...extra });
const off = [{ rule: RULE, line: second.line, endLine: second.line }];

describe("repeatTwins", () => {
  it("removes the first read and keeps everything else, the claim's site included", () => {
    const [request] = repeatTwins({ judged: [asked()], offSite: off, built, program });
    expect(request).toMatchObject({ label, rule: RULE, first: "UNTRACKED_READ_AFTER_AWAIT" });
    expect(request.refused).toBeUndefined();
    // The first read's statement is an inert `1;`; the second read is the one site left, at its own path.
    expect(getAt(request.program, first.path)).toEqual({ k: "num", n: 1 });
    expect(getAt(program, first.path)).toMatchObject({ k: "use", site: true });
    expect(request.built.sites.map((site) => site.path)).toEqual([second.path]);
    expect(request.site.path).toEqual(second.path);
    expect(request.built.text.split("\n")[first.line - 1]).toMatch(/^\s*1;$/);
    expect(request.built.id).toBe(`${built.id}-twin1`);
    expect(request.built.neutral).toBe(true);
  });
  it("asks for no twin of a claim that is not a repeat of a repeat pair", () => {
    expect(repeatTwins({ judged: [asked({ why: "disturbed-repeat" }), { finding: "missing-owner@5", verdict: "stacked", first: "PRIMITIVE_IN_EFFECT_CALLBACK", why: "disturbed" }], offSite: off, built, program })).toEqual([]);
  });
  it("refuses, and a refused claim never holds, where the first read cannot be told", () => {
    const refused = (judged, input = {}) => repeatTwins({ judged, offSite: off, built, program, ...input }).map((request) => request.refused);
    expect(refused([asked({ firstAt: null })])).toEqual(["the first report's read is in 0 sites"]);
    expect(refused([asked({ firstAt: [{ line: 1, column: 1 }] })])).toEqual(["the first report's read is in 0 sites"]);
    expect(refused([asked({ firstAt: [{ line: second.line, column: 1 }] })])).toEqual(["the first read and the claim are one site"]);
    expect(refused([asked({ firstAt: [{ line: first.line, column: 1 }, { line: second.line, column: 1 }] })])).toEqual(["the first report's read is in 2 sites"]);
    expect(refused([asked()], { program: undefined })).toEqual(["no program tree"]);
    expect(refused([asked()], { offSite: [] })).toEqual(["the claim stands on 0 sites"]);
    expect(twinHolds({ refused: "x" }, { checker: { ok: true, findings: [] } }, () => ({ outcome: "agree" }))).toEqual({ holds: false, why: "no twin: x" });
  });
});

describe("twinHolds", () => {
  const request = { label, rule: RULE, first: "UNTRACKED_READ_AFTER_AWAIT", site: second, built, program };
  const value = { typecheck: { ok: true }, checker: { ok: true, findings: [] }, runtime: {} };
  const judged = (outcome, detail = {}) => () => ({ outcome, detail });
  it("holds when the twin judged at the remaining read is an agree through the claim's own rule", () => {
    expect(twinHolds(request, value, judged("agree", { findings: [`${RULE}/violation@${second.line}`] }))).toEqual({ holds: true });
    expect(twinHolds(request, value, judged("agree", { remote: `${RULE}@${second.line + 1}` }))).toEqual({ holds: true });
  });
  it("fails on any other outcome: a false clean, a recall gap, a false positive, a wrong rule, no report", () => {
    for (const outcome of ["false-clean", "recall-gap", "false-positive", "wrong-rule", "not-fired", "stacked-claim", "typed-away", "harness", "misplaced"]) {
      expect(twinHolds(request, value, judged(outcome, { findings: [`${RULE}/violation@${second.line}`] }))).toEqual({ holds: false, why: `twin ${outcome}` });
    }
  });
  it("fails on an agree through another rule, and on a twin that did not run", () => {
    expect(twinHolds(request, value, judged("agree", { findings: ["strict-read-untracked/violation@9"] }))).toEqual({ holds: false, why: "twin agrees through another rule" });
    expect(twinHolds(request, undefined, judged("agree"))).toEqual({ holds: false, why: "the twin did not run" });
    expect(twinHolds(request, { error: "boom" }, judged("agree"))).toEqual({ holds: false, why: "the twin did not run" });
  });
  it("judges the twin as a program of its own: the first code, at the claim's site in the twin, with no other region", () => {
    let seen;
    twinHolds(request, value, (input) => { seen = input; return { outcome: "false-clean" }; });
    expect(seen).toMatchObject({ code: "UNTRACKED_READ_AFTER_AWAIT", site: second, others: [], built, program });
  });
});

// The whole path with the fast runtime's records (Happy DOM: the same dev build that reports a code once per source) and checker findings written from them.
describe("a repeat claim of the aw-helper-late cell, end to end", () => {
  // The fast path records the instrumented twin's positions (a counter in front of each statement moves its columns); the checker's findings are on the clean
  // text, so the grid maps the frames back (hits.mjs `cleanPositions`) before it judges, and so does this.
  const fastRecord = async (root, case_) => {
    const record = (await runFast([{ id: case_.id, dir: writeCase(join(root, case_.id), case_), variant: "run" }], { install: DEFAULT_FAST_INSTALL, workers: 1 })).get(case_.id);
    const clean = cleanPositions(case_.files["App.run.txt"], case_.files["App.tsx"]);
    const place = (positions) => (positions ?? []).flatMap((position) => { const at = clean(position); return at ? [{ ...position, ...at }] : []; });
    return { ...record, diagnostics: record.diagnostics.map((one) => ({ ...one, positions: place(one.positions) })), errors: record.errors.map((one) => ({ ...one, positions: place(one.positions) })) };
  };
  const records = async () => {
    const root = mkdtempSync(join(tmpdir(), "fuzz-repeat-twin-"));
    try {
      const runtime = await fastRecord(root, built);
      const events = runtimeEvents(runtime).filter((event) => event.code === cell.code);
      // The runtime reports the code once, at the first read: the checker's two violations start at the read's own position, each on its own line.
      expect(events.map((event) => event.positions[0].line)).toEqual([first.line]);
      const offset = events[0].positions[0].column - column(built.text, first.line);
      const input = { code: cell.code, site: siteOf(built), typecheck: { ok: true }, checker: { ok: true, findings: [violation(built.text, first.line, RULE, offset), violation(built.text, second.line, RULE, offset)] }, runtime, others: otherRegions(built), built, program };
      const [request] = twinRequests(input);
      expect(request.refused).toBeUndefined();
      const twin = await fastRecord(root, request.built);
      const at = runtimeEvents(twin).find((event) => event.code === cell.code)?.positions[0];
      return { input, request, twin, at };
    } finally { rmSync(root, { recursive: true, force: true }); }
  };
  const valueOf = (request, twin, findings, runtime = twin) => ({ typecheck: { ok: true }, checker: { ok: true, findings }, runtime });
  /** The twin's violation at the one read left, where the twin's runtime recorded it. */
  const read = (request, at, rule = RULE) => violation(request.built.text, request.site.line, rule, at.column - column(request.built.text, request.site.line));

  it("stands as a stacked claim without its twin, and as the agreement it is where the twin holds", async () => {
    const { input, request, twin, at } = await records();
    expect(judge(input)).toMatchObject({ outcome: "stacked-claim", detail: { primary: "agree", pairs: ["UNTRACKED_READ_AFTER_AWAIT|reactive-read-after-await"], at: [second.line] } });
    // The runtime reports the code at the one read left, and a violation covers it.
    expect(runtimeEvents(twin).filter((event) => event.code === cell.code).map((event) => event.positions[0].line)).toEqual([request.site.line]);
    const held = twinHolds(request, valueOf(request, twin, [read(request, at)]), judge);
    expect(held).toEqual({ holds: true });
    const verdict = judge({ ...input, twins: { [request.label]: held } });
    expect(verdict.outcome).toBe("agree");
    expect(verdict.detail).toMatchObject({ twins: { [label]: "held" }, offSite: [`${label}:stacked<UNTRACKED_READ_AFTER_AWAIT(repeat)`] });
  }, 120000);

  it("never turns a false clean twin into agreement: no violation covers the remaining read, the claim stays", async () => {
    const { input, request, twin, at } = await records();
    const failed = twinHolds(request, valueOf(request, twin, []), judge);
    expect(failed).toEqual({ holds: false, why: "twin false-clean" });
    expect(judge({ ...input, twins: { [request.label]: failed } })).toMatchObject({ outcome: "stacked-claim", detail: { primary: "agree", twins: { [label]: "twin false-clean" }, at: [second.line] } });
    // A recall gap twin (the checker declined) is no agreement either.
    const gap = twinHolds(request, valueOf(request, twin, [{ id: "SC9012", rule: "reactive-dispatch-unresolved", kind: "uncertifiable", line: request.site.line, column: at.column, endLine: request.site.line, file: "App.tsx" }]), judge);
    expect(gap).toEqual({ holds: false, why: "twin recall-gap" });
  }, 120000);

  it("never turns a false positive twin into agreement: the runtime reports nothing at the remaining read, the claim stays", async () => {
    const { input, request, twin, at } = await records();
    const silent = { ...twin, diagnostics: [], errors: [] };
    const failed = twinHolds(request, valueOf(request, twin, [read(request, at)], silent), judge);
    expect(failed.holds).toBe(false);
    expect(failed.why).toMatch(/^twin (false-positive|not-fired)$/);
    expect(judge({ ...input, twins: { [request.label]: failed } }).outcome).toBe("stacked-claim");
    // A twin whose violation is of another rule than the claim's is not the claim's twin.
    const other = twinHolds(request, valueOf(request, twin, [read(request, at, "strict-read-untracked")]), judge);
    expect(other.holds).toBe(false);
  }, 120000);

  it("settleRepeats runs the chunk's twins in one batch, judges the row again, and keeps what it ran", async () => {
    const { input, request, twin, at } = await records();
    const entry = { cell, program, built };
    const row = { typecheck: input.typecheck, checker: input.checker, runtime: input.runtime };
    const inputOf = () => input;
    const stub = (findings) => {
      const calls = [];
      return { calls, evaluate: async (entries, config) => { calls.push({ ids: entries.map((one) => one.built.id), casesDir: config.casesDir }); return new Map(entries.map((one) => [one.built.id, valueOf(request, twin, findings)])); } };
    };
    const settle = async (findings, verdict = judge(input)) => {
      const kept = { rows: [], values: new Map() };
      const verdicts = [verdict];
      const probe = stub(findings);
      await settleRepeats({ chunk: [entry], results: new Map([[built.id, row]]), verdicts, inputOf, run: { concurrency: 2 }, out: "/out", kept }, { evaluate: probe.evaluate });
      return { verdict: verdicts[0], kept, calls: probe.calls };
    };
    const held = await settle([read(request, at)]);
    expect(held.verdict.outcome).toBe("agree");
    expect(held.calls).toEqual([{ ids: [request.built.id], casesDir: "/out/twins" }]);
    expect(held.kept.rows).toEqual([{ id: request.built.id, code: "UNTRACKED_READ_AFTER_AWAIT", context: "aw-helper-late", outcome: "twin" }]);
    expect([...held.kept.values.keys()]).toEqual([request.built.id]);
    const failed = await settle([]);
    expect(failed.verdict).toMatchObject({ outcome: "stacked-claim", detail: { twins: { [label]: "twin false-clean" } } });
    // A row that is not a stacked claim asks for no twin and no evaluation.
    const other = await settle([], { outcome: "agree", detail: {} });
    expect(other.verdict).toEqual({ outcome: "agree", detail: {} });
    expect(other.calls).toEqual([]);
  }, 120000);
});
