// Batch 11, slice 0 (measurement only): how the candidate sites of the rules that have no site universe end. The checker's dry run (SOLID_CHECKER_IR_PROOF_CANDIDATES=1, with the proof log on;
// rust/crates/solid-reactive-ir/src/proof_candidates.rs) writes one `candidate` line per place a producer's own first test holds. This fold reads each one's ending from the same log's final
// findings (`verdict` lines), named silences (`silent`) and facts, and changes nothing:
//
//   finding       a finding of the rule itself stands at the site (`violation` or `obligation`): the rule answered
//   other         findings stand there and none is the rule's: the site ends today only because ANOTHER rule reported (the any-rule-credit list)
//   silent        a named silence stands at the span (a `silent` line): the silence of ANOTHER account's site at the same place (the owner accounting's, a universe's own), which ends that site and
//                 says nothing of this rule's claim: a count of `silent` is no accounting of the rule
//   discarded     the compiler deleted the code (the candidate's `discarded` fact)
//   unaccounted   nothing stands there: a closed accounting would have to end it, in a row or a typed silence, or it would become an uncertifiable row
//
// `unaccounted` is an UPPER bound of the rows a universe would add: a site that is provably outside the rule's claim (a `resolve()` in an event handler, a `createMemo` with no `sync` option) ends
// here as `unaccounted` until a typed silence names the proof, and the dry run names none. The facts of each candidate (a `sync` option's state, whether `on` is proven not to be a function)
// split the count where the producer's first test already tells the sites apart.
//
// Two rules have no call to start from and are read from the log's own lines: SC3001's candidates are the owner accounting's sites that may run under a leaf owner (`owner` lines whose
// context holds the leaf bit, 4, or the leaf-owned bit, 64: a finding of SC3001 inside the call is the rule's own), and SC2009's the optimistic setter calls (the write universe's silences
// `push_setter_write.optimistic-*-unguarded`, and the sites of SC2009's own findings). SC1002 and SC6001 are not measured.
import { proofLogLines } from "./proof-log.mjs";

const spanKey = (s, e) => `${s}:${e}`;

/** The context bits of `owners.rs` that put a site under a leaf owner. */
export const OWNER_CONTEXT_LEAF = 4;
export const OWNER_CONTEXT_LEAF_OWNED = 64;

const OPTIMISTIC_SILENCES = new Set(["local_access.push_setter_write.optimistic-signal-unguarded", "local_access.push_setter_write.optimistic-store-unguarded"]);

/** The ids of the findings that answer each rule's candidates (`flush` is the census, which both flush rules answer). */
const ANSWERS = { flush: ["SC2006", "SC2008"] };
const answersOf = (rule) => ANSWERS[rule] ?? [rule];

/** Whether a path is a test file: `*.test.*`, `*.spec.*`, or under `__tests__` / `__mocks__`. */
export const isTestFile = (path) => /(^|\/)__(tests|mocks)__\//.test(path) || /\.(test|spec)\.[cm]?[jt]sx?$/.test(path);

const within = (outer, inner) => outer.s <= inner.s && inner.e <= outer.e;

/**
 * The candidates of one log file's lines, each with how it ends: `[{ file, program, rule, table, s, e, facts, end, ownRule, otherRules }]`.
 * `end` is one of `finding-violation`, `finding-obligation`, `other`, `silent`, `discarded`, `unaccounted`.
 */
export function candidatesOf(lines, { orphans = null, stats = null } = {}) {
  const files = new Map();
  const of = (file) => {
    if (!files.has(file)) files.set(file, { program: null, candidates: [], verdicts: [], bySpan: new Map(), silent: new Set(), owners: [], optimistic: new Set() });
    return files.get(file);
  };
  for (const line of lines) {
    if (line.f === undefined) continue;
    if (line.k === "candidate") {
      if (stats) stats.candidateLines += 1;
      const entry = of(line.f);
      entry.program ??= line.g ?? null;
      entry.candidates.push({ rule: line.rule, table: line.t, s: line.s, e: line.e, alt: line.alt ?? [], facts: line.facts ?? {} });
    } else if (line.k === "verdict") {
      const entry = of(line.f);
      entry.program ??= line.g ?? null;
      const verdict = { s: line.s, e: line.e, code: line.code, rule: line.rule ?? null, kind: line.kind, ctx: line.ctx ?? "" };
      entry.verdicts.push(verdict);
      const key = spanKey(line.s, line.e);
      if (!entry.bySpan.has(key)) entry.bySpan.set(key, []);
      entry.bySpan.get(key).push(verdict);
    } else if (line.k === "silent") {
      const entry = of(line.f);
      entry.silent.add(spanKey(line.s, line.e));
      if (OPTIMISTIC_SILENCES.has(line.proof)) entry.optimistic.add(spanKey(line.s, line.e));
    } else if (line.k === "owner" && ((line.ctx ?? 0) & (OWNER_CONTEXT_LEAF | OWNER_CONTEXT_LEAF_OWNED)) !== 0) {
      of(line.f).owners.push({ s: line.s, e: line.e, ctx: line.ctx });
    }
  }
  const out = [];
  for (const [file, entry] of files) {
    const candidates = [...entry.candidates];
    // SC3001: the sites that may run under a leaf owner; SC2009: the optimistic setter calls (a silence at the site, or a finding of the rule).
    for (const owner of entry.owners) candidates.push({ rule: "SC3001", table: "leaf-owner-site", s: owner.s, e: owner.e, alt: [], facts: { leaf: (owner.ctx & OWNER_CONTEXT_LEAF) !== 0 ? "yes" : "no", leafOwned: (owner.ctx & OWNER_CONTEXT_LEAF_OWNED) !== 0 ? "yes" : "no" } });
    const optimistic = new Map();
    for (const key of entry.optimistic) { const [s, e] = key.split(":").map(Number); optimistic.set(key, { s, e }); }
    for (const verdict of entry.verdicts) if (verdict.code === "SC2009") optimistic.set(spanKey(verdict.s, verdict.e), { s: verdict.s, e: verdict.e });
    for (const { s, e } of optimistic.values()) candidates.push({ rule: "SC2009", table: "optimistic-write", s, e, alt: [], facts: {} });
    for (const candidate of candidates) {
      const spans = [spanKey(candidate.s, candidate.e), ...candidate.alt.map(([s, e]) => spanKey(s, e))];
      // the findings at the candidate's spans; for a rule with no single span (SC3001's operation is reached through helpers, the census holds the whole call) the findings inside it
      const contained = candidate.rule === "SC3001" || candidate.rule === "flush";
      const found = contained ? entry.verdicts.filter((verdict) => within(candidate, verdict)) : spans.flatMap((key) => entry.bySpan.get(key) ?? []);
      const answers = answersOf(candidate.rule);
      const own = found.filter((verdict) => answers.includes(verdict.code));
      const others = found.filter((verdict) => !answers.includes(verdict.code));
      let end;
      if (own.length) end = own.some((verdict) => verdict.kind === "violation") ? "finding-violation" : "finding-obligation";
      else if (others.length) end = "other";
      else if (spans.some((key) => entry.silent.has(key)) && candidate.rule !== "SC2009") end = "silent";
      else if (candidate.facts.discarded === "yes") end = "discarded";
      else end = "unaccounted";
      out.push({ file, program: entry.program, rule: candidate.rule, table: candidate.table, s: candidate.s, e: candidate.e, facts: candidate.facts, end, ownRule: own.map((verdict) => verdict.code), otherRules: [...new Set(others.map((verdict) => verdict.rule ?? verdict.code))] });
    }
    // How well the candidates cover the rules' own findings: a finding of a rule that has candidates and stands at none of them (not at a candidate's place, nor inside it for the rules read by containment)
    if (orphans) {
      const covered = new Set(candidates.flatMap((candidate) => answersOfRule(candidate.rule).map((rule) => `${rule}|${spanKey(candidate.s, candidate.e)}`).concat(candidate.alt.flatMap(([s, e]) => answersOfRule(candidate.rule).map((rule) => `${rule}|${spanKey(s, e)}`)))));
      const listed = new Set(candidates.flatMap((candidate) => answersOfRule(candidate.rule)));
      for (const verdict of entry.verdicts) {
        if (!listed.has(verdict.code)) continue;
        // A finding about a dependency package's own behaviour is reported where project code enters the package: not at a call of the project's, so no candidate of this list is there to cover it
        if (String(verdict.ctx).startsWith("package-finding-at-entry")) {
          orphans.packageAtEntry[verdict.code] = (orphans.packageAtEntry[verdict.code] ?? 0) + 1;
          continue;
        }
        orphans.findings[verdict.code] = (orphans.findings[verdict.code] ?? 0) + 1;
        const here = covered.has(`${verdict.code}|${spanKey(verdict.s, verdict.e)}`)
          || candidates.some((candidate) => (candidate.rule === "SC3001" || candidate.rule === "flush") && answersOfRule(candidate.rule).includes(verdict.code) && within(candidate, verdict));
        if (!here) orphans.uncovered[verdict.code] = (orphans.uncovered[verdict.code] ?? 0) + 1;
      }
    }
  }
  return out;
}

/** The finding ids that answer a candidate rule (`flush` is answered by both flush rules). */
const answersOfRule = (rule) => answersOf(rule);

const ENDS = ["finding-violation", "finding-obligation", "other", "silent", "discarded", "unaccounted"];
const emptyEnds = () => Object.fromEntries(ENDS.map((end) => [end, 0]));

/** An empty tally: the candidates, how each ended, which rules ended `other`, the split by each fact's value, and the candidates in test files. */
export const newTally = () => ({ candidates: 0, ends: emptyEnds(), otherBy: {}, byFact: {}, inTestFiles: { candidates: 0, ends: emptyEnds() } });

/** The facts of a candidate and the ones that join two of them: an SC1002 read's `shape` says whether its async function is written as a computation's function and whether the read is directly in it. */
export function factsOf(candidate) {
  const facts = { ...candidate.facts };
  if (candidate.rule === "SC1002" && facts.computation !== undefined && facts.nested !== undefined) facts.shape = `${facts.computation === "yes" ? "computation" : "other-async"}-${facts.nested === "yes" ? "nested" : "direct"}`;
  return facts;
}

/** Adds one candidate (from `candidatesOf`) to a tally. */
export function addCandidate(tally, candidate) {
  const count = (target) => { target.candidates += 1; target.ends[candidate.end] += 1; };
  count(tally);
  if (candidate.end === "other") for (const rule of candidate.otherRules) tally.otherBy[rule] = (tally.otherBy[rule] ?? 0) + 1;
  for (const [name, value] of Object.entries(factsOf(candidate))) {
    if (name === "discarded") continue;
    const key = `${name}=${value}`;
    tally.byFact[key] ??= { candidates: 0, ends: emptyEnds() };
    tally.byFact[key].candidates += 1;
    tally.byFact[key].ends[candidate.end] += 1;
  }
  if (isTestFile(candidate.file)) { tally.inTestFiles.candidates += 1; tally.inTestFiles.ends[candidate.end] += 1; }
}

/** The tallies of `candidates` per `<rule> <table>`: `{ "SC2004 resolve": tally, ... }`. */
export function tallyCandidates(candidates, into = {}) {
  for (const candidate of candidates) addCandidate((into[`${candidate.rule} ${candidate.table}`] ??= newTally()), candidate);
  return into;
}

/** Adds `tally` into `into` (the per-project tallies into the corpus total). */
export function mergeTally(into, tally) {
  into.candidates += tally.candidates;
  for (const end of ENDS) { into.ends[end] += tally.ends[end]; into.inTestFiles.ends[end] += tally.inTestFiles.ends[end]; }
  into.inTestFiles.candidates += tally.inTestFiles.candidates;
  for (const [rule, count] of Object.entries(tally.otherBy)) into.otherBy[rule] = (into.otherBy[rule] ?? 0) + count;
  for (const [key, fact] of Object.entries(tally.byFact)) {
    into.byFact[key] ??= { candidates: 0, ends: emptyEnds() };
    into.byFact[key].candidates += fact.candidates;
    for (const end of ENDS) into.byFact[key].ends[end] += fact.ends[end];
  }
  return into;
}

/**
 * The candidates of one log file, read in chunks and folded: `{ program, tallies, orphans, candidateLines }`. `candidateLines` counts the dry run's own `candidate` lines (not the
 * candidates read from the `owner` and `silent` lines): a log with none is a program whose analysis ended before the accounting, or a run with the dry run off.
 */
export function foldLogFile(path) {
  const orphans = { findings: {}, uncovered: {}, packageAtEntry: {} };
  const stats = { candidateLines: 0 };
  const candidates = candidatesOf(proofLogLines(path), { orphans, stats });
  return { program: candidates.find((candidate) => candidate.program)?.program ?? null, tallies: tallyCandidates(candidates), orphans, candidateLines: stats.candidateLines };
}
