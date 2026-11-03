// Invariant 1 (AGENTS.md): the checker never reports what `tsc` reports. A program `tsrs` rejects is dropped as evidence
// (`typed-away`), but the grids still run the checker on it (`checkTypedAway` below), and this module says whether a checker
// violation sits on code the type check rejected. Such a program is `tsc-overlap`: a candidate for a person, never dropped. It is not
// a verdict by itself: "a different claim about the same code (ownership, timing) is allowed", so the owner reads the pair (the TS
// error, the violation) and either removes the rule's case (a duplicate of tsc, recorded in fixtures/tsc-oracle) or accepts the claim
// as a different one by an entry in ACCEPTED below, with its reason.

import { rmSync } from "node:fs";
import { mapPoolSettled } from "../lib/pool.mjs";
import { checkerProjectFiles, checkerResult, checkerWorkDir, materializeForChecker, runChecker } from "./checker.mjs";
import { costOf } from "./cost.mjs";
import { hasPackages } from "./pipeline.mjs";

/** A type-check error line as `typecheck.mjs` returns it: `App.tsx(12,5): error TS2345: ...`. */
const ERROR_LINE = /^(.*?)\((\d+),(\d+)\): error (TS\d+): (.*)$/;

/** The errors of a failed type check in `file` (the program's App.tsx unless named): `{ line, column, code, message }`. */
export function typeErrorsIn(typed, file = "App.tsx") {
  return (typed?.errors ?? []).flatMap((text) => {
    const match = ERROR_LINE.exec(String(text));
    if (!match || match[1].replace(/^\.\//, "") !== file) return [];
    return [{ line: Number(match[2]), column: Number(match[3]), code: match[4], message: match[5] }];
  });
}

/**
 * Pairs a person has read and judged a different claim from the type error's: `{ ts, rule, message }` (the TS code, the rule, a
 * pattern on the TS message), each with its reason. Empty: none was needed.
 */
export const ACCEPTED = [];
const accepted = (error, finding) => ACCEPTED.find((entry) => entry.ts === error.code && entry.rule === finding.rule && entry.message.test(error.message));

/**
 * The tsc overlap of a typed-away program: the checker's violations (every rule, the style rules too: they are reports like any other)
 * whose line range holds a line the type check rejected, and, when the rejected line is the site's own, a violation of a rule that
 * claims the site's code anywhere in the file (the checker anchors a misuse at the call that runs it, which the judges call `remote`).
 * @param {{ typed: object, checker: object, region?: number[], rules?: string[] }} input
 * @returns {{ overlaps: string[], accepted: string[], elsewhere: string[], errors: string[] }}
 */
export function tscOverlap({ typed, checker, region = [], rules = [] }) {
  const errors = typeErrorsIn(typed);
  const violations = (checker?.ok ? checker.findings : []).filter((finding) => finding.kind === "violation");
  const overlaps = []; const acceptedPairs = []; const elsewhere = [];
  for (const finding of violations) {
    const label = `${finding.rule}@${finding.line}`;
    const spans = errors.filter((error) => finding.line <= error.line && error.line <= finding.endLine);
    const remote = rules.includes(finding.rule) ? errors.filter((error) => region.includes(error.line)) : [];
    const hit = [...spans, ...remote];
    if (!hit.length) { elsewhere.push(label); continue; }
    if (hit.every((error) => accepted(error, finding))) acceptedPairs.push(`${label}~${hit[0].code}`);
    else overlaps.push(`${label}~${hit.find((error) => !accepted(error, finding)).code}@${hit[0].line}`);
  }
  return { overlaps, accepted: acceptedPairs, elsewhere, errors: errors.map((error) => `${error.code}@${error.line}`) };
}

/**
 * The verdict of a typed-away program in a grid: `tsc-overlap` when a violation overlaps the rejected code, else `typed-away` with the
 * checker's other answers listed (a violation elsewhere in the file is not tsc's claim and is not judged here).
 */
export function typedAwayVerdict({ typed, checker, region, rules }) {
  const error = typed.errors?.[0];
  if (!checker) return { outcome: "typed-away", detail: { error } };
  if (!checker.ok) return { outcome: "typed-away", detail: { error, checker: `failed: ${String(checker.error ?? "").slice(0, 120)}` } };
  const found = tscOverlap({ typed, checker, region, rules });
  const listed = { errors: found.errors, ...(found.accepted.length ? { accepted: found.accepted } : {}), ...(found.elsewhere.length ? { elsewhere: found.elsewhere } : {}) };
  if (found.overlaps.length) return { outcome: "tsc-overlap", detail: { error, overlaps: found.overlaps, ...listed } };
  return { outcome: "typed-away", detail: { error, ...listed } };
}

/**
 * Runs the checker on the evaluated programs `tsrs` rejected (`results`, pipeline.mjs's map), which step 1 left unchecked: through the
 * Rust executor when the run has one (it stops at a failed type check, so this is a second call, as for a type check's spot check),
 * else on a project written for it. Each gets `checker` (and `cost`); no runtime runs on it. Outside pipeline.mjs on purpose: that file
 * keys the runtime cache (cache.mjs RECORD_SOURCES), and this changes no record. `deps.runChecker` is for tests.
 */
export async function checkTypedAway(results, config, deps = {}) {
  const check = deps.runChecker ?? runChecker;
  const pending = [...results.values()].filter((value) => value.typecheck && value.typecheck.ok === false && !value.checker && !value.error);
  const exec = config.exec && !deps.runChecker ? await config.exec : null;
  const settled = await mapPoolSettled(pending, async (value) => {
    const { built } = value;
    const ordinary = hasPackages(built);
    if (exec) {
      const own = ordinary || config.keepWork;
      const answer = await exec.prepare({ id: built.id, caseWrites: [], project: { files: checkerProjectFiles(built), ...(own ? { dir: checkerWorkDir(config.install, built.id) } : {}) }, twinProject: null, clean: false, twin: false, checker: { ordinary } });
      value.checker = checkerResult(answer.checker, answer.checker.dir);
      value.cost = costOf(answer.checker);
      return;
    }
    const dir = materializeForChecker(built, config.install, built.id);
    try { value.checker = await check(dir, { bin: config.bin, typefacts: config.typefacts, ordinary }); } finally { if (!config.keepWork) rmSync(dir, { recursive: true, force: true }); }
  }, { concurrency: Math.max(1, config.concurrency ?? 1) });
  // A failed run is the program's harness answer, never a silent skip.
  settled.forEach((result, index) => { if (result.error) pending[index].checker = { ok: false, findings: [], error: String(result.error.message ?? result.error) }; });
  return pending.length;
}
