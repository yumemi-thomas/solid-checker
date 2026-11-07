#!/usr/bin/env node
// The recall matrix of the misuse profile (misuse.mjs): for every code a rule claims, every context the template targets, N programs
// that misuse one Solid API at one marked site. Each program is judged at its site, against the runtime that confirms the misuse:
//
//   programs         generated for the cell
//   typed-away       tsc rejects the program (invariant 1: the checker stays silent; not a gap)
//   tsc-overlap      tsc rejects the program and a checker violation covers the rejected code (tsc-overlap.mjs): a candidate breach
//                    of invariant 1 for a person to read, counted as a problem (focus.mjs PROBLEMS) and by the gate
//   harness          the checker or the runtime run failed (not evidence)
//   not-fired        the program type-checks and the runtime did not report the code at the site: a generator problem, not the checker's
//   confirmed        the runtime reported the code at the site (the only programs that count in the checker columns)
//     agree          a violation of a rule that claims the code covers the site
//     wrong-rule     a violation of another rule covers the site and none of the claiming rules does
//     recall-gap     only uncertifiable findings cover the site (the checker declined)
//     false-clean    no finding covers the site: the checker is silent where Solid speaks
//
// Chrome (the authority) confirms up to --chrome-sample agree programs per cell and every wrong-rule and false-clean program; one
// representative per false-clean cell is shrunk. `--baseline <matrix.json>` gates: a cell whose agree count drops or whose false-clean
// count rises, for the same seeds, fails the run (exit 1). `--gate <baseline.json> <current.json>` compares two recorded matrices.
//
//   node scripts/fuzz/matrix.mjs --out <fresh dir> --seed 1 --per-cell 20 [--codes A,B] [--contexts a,b] [--confirm-with-chrome]
//     [--chrome-sample 3] [--shrink] [--baseline matrix.json] [--checker-label <commit>] [--all-cores]
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { STORED_ASYNC_CONTEXTS, STORED_CONTEXTS } from "./flip-read.mjs";
import { guardGridExit, partialRows } from "./partial.mjs";
import { createHash } from "node:crypto";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { browserExecutionCache, withRuntimeCache, cacheStats } from "./cache.mjs";
import { callStarts, frameStarts, statementAt } from "./call-starts.mjs";
import { registrationPositions } from "./registration.mjs";
import { partlySilentRules, besideCode, codeOfLogged, codeOfUncaught, codeRules, codesForRule, coversCodeAt, declinesCode, appStack, withheldAtStack, withStarts, ignoredRules, isPlacelessCode, policyOf, thrownCodes, withholdingRules } from "./codes.mjs";
import { offSiteVerdicts, runtimeEvents } from "./classify.mjs";
import { pairOf, stackedSplit } from "./stacked.mjs";
import { hitCount } from "./hits.mjs";
import { DEFAULT_ORACLE_INSTALL, runChrome } from "./chrome.mjs";
import { fnv } from "./rng.mjs";
import { chromeUnreadable, evidenceOf, parseAxes, parsePerStratum, runtimeDiff, sampleStrata, stratumOf, summarize } from "./runtime-diff.mjs";
import { buildCase, writeCase } from "./emit.mjs";
import { GENERATOR_VERSION, generateProgram } from "./generate.mjs";
import {
  BASE_ONLY_CONTEXTS, CALLBACK_CONTEXTS, CALLBACK_FORMS, CBP_CONTEXTS, LAYERS, PAIR_CALLBACKS, PAIR_LAYERS, PAIR_MOUNTS, PAIR_SETS, PAIR_SOURCES, PROMISE_SWALLOWS, legalVariantsOf, variantsOf, MISUSE_CODES, MOUNTS, RECALL_LAYERS, RECALL_TEMPLATE_CODES, SOURCE_AXES, SPEC, TEMPLATE_CODES,
  gridBindingsOf, gridVariantsOf, recallBindingsOf, recallContextsOf, recallLegalOf, recallVariantsOf, bindingsOf, sourcesOf, sourcesOption, fireShape } from "./misuse.mjs";
import { LATER_MOUNTS, RECALL_MOUNTS } from "./mounts.mjs";
import { HANDOFF_FORMS, isSite, walk } from "./model.mjs";
import { evaluate, finishEntries, formatPhaseTimings, phaseTimings, preparedAhead, prepareEntries, runtimeCached } from "./pipeline.mjs";
import { budgetOf, costLine, costLines, writeCosts } from "./cost.mjs";
import { gridProfile } from "./perf.mjs";
import { programSeed } from "./rng.mjs";
import { shrink } from "./shrink.mjs";
import { legalTwin } from "./falseclean.mjs";
import { attachDaemon, configFrom, parseArgs } from "./run.mjs";
import { setExecutorChecks } from "./exec.mjs";
import { linkScratch } from "./scratch.mjs";
import { idFilter, keepIds } from "./smoke.mjs";
import { gridSampleOptions, sampleEntries, sampleLine } from "./sample.mjs";
import { gridShardOptions, shardEntries, shardKey, shardLine, shardOf } from "./shard.mjs";
import { assertFindingsMapped, materializeForChecker } from "./checker.mjs";
import { checkTypedAway, typeRejectedRows, typedAwayVerdict, typedOnPurpose } from "./tsc-overlap.mjs";
import { TYPE_ERROR_REFUSAL_SHARE, harnessRefusal, requireTypeChecker, resolveTsc, typeErrorRefusal, typecheck } from "./typecheck.mjs";

export const OUTCOMES = ["typed-away", "harness", "unexecuted", "pending-uncaught", "not-fired", "agree", "wrong-rule", "recall-gap", "false-clean", "misplaced", "unplaced", "unjudged-claim", "stacked-claim", "false-positive", "tsc-overlap"];
const CONFIRMED = ["agree", "wrong-rule", "recall-gap", "false-clean", "false-positive"];

// ---------------------------------------------------------------------------------------------------------------------
// Judging one program at its site

const inLines = (finding, lines) => lines.some((line) => finding.line <= line && line <= finding.endLine);
/**
 * A rule that stands beside a code (codes.mjs `besideCode`, ADR 0483) makes a claim that code's throw hides, whichever code the row was generated
 * for: the row's own, or any code the run recorded on the finding's lines (ADR 0887 amended: `createEffect(..., { sync: true })` in an apply over an
 * async source records SYNC_NODE_RECEIVED_ASYNC and PRIMITIVE_IN_EFFECT_CALLBACK at one line, and the row's code is only the first).
 */
const standsBeside = (code, finding, runtime) => Boolean(besideCode[code]?.includes(finding.rule))
  || runtimeEvents(runtime).some((event) => besideCode[event.code]?.includes(finding.rule)
    && event.positions.some((position) => (position.file === "App.tsx" || position.file === undefined) && inLines(finding, [position.line])));

/** The lines that belong to the site: where the misuse call is written, and the line of a wrapper's inner call. */
export const regionOf = (site) => [site.line, ...(site.aux ?? [])].filter((line) => Number.isInteger(line));

/**
 * `built` (the program as printed: its sites, glue and plan; `program`, its tree, for registration lines) lets the violations away from the site be judged too (classify.mjs
 * `offSiteVerdicts`): one that ran where the runtime reported no code of its rule turns an `agree` or a `recall-gap` into a
 * `false-positive` (`detail.primary` keeps the site's own outcome). A remote claiming violation stays the site's agreement.
 * @param {{ code: string, site: object, typecheck: object, checker: object, runtime: object, built?: object }} input
 * @returns {{ outcome: string, detail?: object }}
 */
export function judge(input) {
  const verdict = judgeSite(input);
  const { site, checker, runtime, built, program } = input;
  if (!built || !checker?.ok || !runtime || !["agree", "recall-gap", "not-fired", "wrong-rule", "false-clean"].includes(verdict.outcome)) return verdict;
  const region = regionOf(site);
  // ADR 0614: the site's agreement answers for the row's own code, nothing else. Another rule's violation on the site's lines is judged like one
  // away from them, and on a `not-fired` program so is the claiming rule's own: its site ran and the runtime reported nothing, which is what a
  // false positive looks like, not a generator problem.
  const claiming = codeRules[input.code] ?? [];
  // A rule that stands beside the row's code (codes.mjs `besideCode`, ADR 0483) makes a claim the code's throw hides: the runtime stops first.
  const judgedAtSite = (finding) => verdict.outcome === "not-fired" || !(claiming.includes(finding.rule) || standsBeside(input.code, finding, runtime));
  const offSite = checker.findings.filter((finding) => finding.kind === "violation" && !ignoredRules.includes(finding.rule) && (!inLines(finding, region) || judgedAtSite(finding)));
  const judged = offSiteVerdicts({ findings: offSite, built, program, runtime, plan: built.plan }).filter((row) => row.verdict !== "substantiated" && !(row.verdict === "unjudged" && row.why === "no-counterpart"));
  if (!judged.length) return verdict;
  const detail = { ...verdict.detail, offSite: judged.map((row) => `${row.finding}:${row.verdict}${row.first ? `<${row.first}` : ""}${row.why ? `(${row.why})` : ""}`) };
  // A stacked violation (classify.mjs `stacked`: Solid's first throw halted the run on its lines) is a false positive unless its pair is
  // proved (stacked.mjs), then a `stacked-claim`, listed by id with its pairs.
  const { wrong, stacked } = stackedSplit(judged);
  // A false clean at the site is already a problem: the off-site false positive is listed beside it. A wrong rule is not above a false
  // positive: a violation the runtime contradicts is one whatever else the row found, and `beside` is an `agree` with that violation in it.
  const scored = ["agree", "recall-gap", "not-fired", "wrong-rule"].includes(verdict.outcome);
  const primary = verdict.detail?.beside ? "agree" : verdict.outcome;
  if (wrong.length && scored) return { outcome: "false-positive", detail: { ...detail, primary, rules: [...new Set(wrong.map((row) => row.finding.split("@")[0]))], at: wrong.map((row) => Number(row.finding.split("@")[1])) } };
  if (stacked.length && scored) return { outcome: "stacked-claim", detail: { ...detail, primary, pairs: [...new Set(stacked.map(pairOf))].sort(), at: stacked.map((row) => Number(row.finding.split("@")[1])) } };
  // A violation the rule's code may confirm but Chrome gave it no line (`unplaced`): unjudged, listed by id, never cleared (Part 16).
  const unplaced = judged.filter((row) => row.verdict === "unjudged" && /(?:^|-)unplaced$/.test(row.why ?? ""));
  if (unplaced.length && scored) return { outcome: "unjudged-claim", detail: { ...detail, primary, rules: [...new Set(unplaced.map((row) => row.finding.split("@")[0]))], at: unplaced.map((row) => Number(row.finding.split("@")[1])) } };
  return { outcome: verdict.outcome, detail };
}

/** The line of a diagnostic's own `loc` (`…App.tsx:12:25`; a grid slot's prefix differs), or null. */
const locLine = (event) => {
  const match = /App\.tsx:(\d+):/.exec(String(event.loc ?? ""));
  return match ? Number(match[1]) : null;
};

/**
 * The lines a placeless code's `loc` may name for this site: the opening line of each node enclosing the site (a proper prefix of its
 * path in `built.lineTable`) whose span reaches the site's line. Solid's ASYNC_OUTSIDE_LOADING_BOUNDARY carries the `{` of the JSX hole
 * of the render effect that raised it, which holds the site; its stack is the flush's and names no App.tsx line. A node that also holds
 * another use site of the program (`others`) names no one site, so it is not one of them.
 */
const holeLines = (built, site, others = []) => {
  const path = site?.path;
  if (!built?.lineTable || !Array.isArray(path) || !Number.isInteger(site.line)) return [];
  return built.lineTable
    .filter((row) => Array.isArray(row.path) && row.path.length < path.length && row.path.every((key, index) => key === path[index]) && row.endLine >= site.line && row.line <= site.line)
    .filter((row) => !others.some((lines) => lines.some((line) => row.line <= line && line <= row.endLine)))
    .map((row) => row.line);
};

function judgeSite({ code, site, typecheck: typed, checker, runtime, others = [], built }) {
  // A program tsc rejects is no evidence of recall, but a checker violation on the rejected code is surfaced (tsc-overlap.mjs).
  if (typed && typed.ok === false) return typedAwayVerdict({ typed, checker, region: regionOf(site), rules: codeRules[code] ?? [] });
  if (!checker?.ok) return { outcome: "harness", detail: { why: `checker: ${String(checker?.error ?? "not run").slice(0, 120)}` } };
  if (!runtime) return { outcome: "harness", detail: { why: "no runtime record" } };
  const region = regionOf(site);
  // The coded errors the page threw: `[CODE] message` is the dev build's own tag (Chrome reports some codes only this way).
  const thrownTags = (runtime.errors ?? []).flatMap((error) => {
    const tag = /^\[([A-Z][A-Z_]+)\]/.exec(String(error.message ?? ""));
    return tag ? [{ code: tag[1], source: "thrown", positions: error.positions ?? [] }] : [];
  });
  // A thrown code an `<Errored>` boundary caught: the boundary logs it (`console.error(err())`, solid-js rc.17 `solid.dev.js:1647-1652`)
  // instead of rethrowing it, with no position, so it judges as unplaced.
  const loggedTags = (runtime.consoleErrors ?? []).flatMap((text) => {
    const logged = codeOfLogged(text);
    return logged ? [{ code: logged, source: "thrown", positions: [] }] : [];
  });
  // ADR 0858: an uncaught throw that names a Solid code is the program's own runtime result, where Chrome placed it; a page halted by one (a
  // later step found no element) carries that throw's code. Any other failure is the harness's.
  const failure = runtime.failure ?? null;
  const failureCode = failure ? codeOfUncaught(failure.message) ?? (failure.phase === "halted" ? (runtime.errors ?? []).map((error) => codeOfUncaught(error.message)).find(Boolean) ?? null : null) : null;
  const failureTags = failureCode && codeOfUncaught(failure.message) ? [{ code: failureCode, source: "thrown", positions: failure.positions ?? [] }] : [];
  const known = runtimeEvents(runtime);
  const events = [...known, ...[...thrownTags, ...loggedTags, ...failureTags].filter((tag) => !known.some((event) => event.code === tag.code && event.source === "thrown"))].filter((event) => event.code === code);
  const attributed = (event) => event.positions.filter((position) => position.file === "App.tsx" || position.file === undefined);
  const atSite = events.filter((event) => attributed(event).some((position) => region.includes(position.line)));
  // A diagnostic with no authored position at all (or, for a placeless code, any position): Solid spoke and the harness cannot say
  // where. With one site in the file that is this site. Chrome lists such codes in `unattributed`.
  const unplaced = events.filter((event) => SPEC[code].placeless || !attributed(event).length);
  const unattributed = (runtime.unattributed ?? []).map(String).includes(code);
  // A combination program's other uses are legal sites of other codes: a finding on their lines is theirs, never this site's.
  const elsewhere = (finding) => others.some((lines) => inLines(finding, lines)) && !inLines(finding, region);
  const relevant = checker.findings.filter((finding) => !ignoredRules.includes(finding.rule) && !elsewhere(finding));
  // Another claimed code Chrome reports on the site's lines (a store link written in a body is a strict read too): the checker
  // missed it when no finding of its rules is in the file (`false-clean`), or declined it (`recall-gap`). The target's outcome
  // does not see it, so it is counted on its own, on a `not-fired` program too (ADR 0615).
  const secondary = Object.fromEntries([...new Set(known
    .filter((event) => event.code !== code && (codeRules[event.code] ?? []).length && attributed(event).some((position) => region.includes(position.line)))
    .map((event) => event.code))]
    .map((other) => [other, relevant.some((finding) => finding.kind === "violation" && codeRules[other].includes(finding.rule))
      ? "agree"
      : relevant.some((finding) => codeRules[other].includes(finding.rule) || declinesCode(other, finding)) ? "recall-gap"
      // In a file with no uncertifiable finding the checker claims it proved everything: a certified false clean (invariant 2), which
      // focus.mjs fails like the target's own (`secondary-false-clean`).
      : checker.findings.some((finding) => !ignoredRules.includes(finding.rule) && finding.kind === "uncertifiable") ? "false-clean" : "false-clean-certified"])
    .filter(([, verdict]) => verdict !== "agree"));
  if (!atSite.length && !unplaced.length && !unattributed) {
    // ADR 0815: Solid's NotReadyError, uncaught, ended the page: a pending read with nothing to hold it at a mount (`render(f1, el)` with
    // `f1` reading a pending memo). It has no code and an empty message, and it is the program's result, not the harness's: judged by the
    // rows at the site, whose read Chrome's stack cannot place (the error's stack is the memo's creation).
    if (runtime.failure?.name === "NotReadyError" && !events.length) {
      const atRead = relevant.filter((finding) => inLines(finding, region) && ((codeRules[code] ?? []).includes(finding.rule) || withholdingRules.includes(finding.rule) || finding.rule === "pending-async-unsuspendable-read" || finding.rule === "strict-read-untracked"));
      return atRead.length ? { outcome: "pending-uncaught", detail: { findings: atRead.map((finding) => `${finding.rule}/${finding.kind}@${finding.line}`) } } : { outcome: "false-clean", detail: { uncaughtPending: true, findings: relevant.filter((finding) => inLines(finding, region)).map((finding) => `${finding.rule}/${finding.kind}@${finding.line}`), certified: !relevant.some((finding) => finding.kind === "uncertifiable") } };
    }
    if (runtime.failure && !events.length && !failureCode) return { outcome: "harness", detail: { why: `runtime: ${String(runtime.failure.message).slice(0, 120)}` } };
    // ADR 0858: the throw halted the page before the site ran: nothing was measured there, neither a pass nor a problem.
    // Review R14: the halting code is judged anywhere in the file, or a false clean of it would read as "unexecuted".
    if (failureCode && !events.length && hitCount(runtime.hits, site?.hit) === 0) {
      const covered = !(codeRules[failureCode] ?? []).length || relevant.some((finding) => codeRules[failureCode].includes(finding.rule) || declinesCode(failureCode, finding));
      const haltSecondary = covered ? {} : { secondary: { [failureCode]: relevant.some((finding) => finding.kind === "uncertifiable") ? "false-clean" : "false-clean-certified" } };
      return { outcome: "unexecuted", detail: { why: `the page halted on ${failureCode} before the site ran`, ...haltSecondary } };
    }
    return { outcome: "not-fired", detail: { reported: [...new Set(runtimeEvents(runtime).map((event) => event.code))], misplaced: events.map((event) => attributed(event).map((p) => p.line)), ...(Object.keys(secondary).length ? { secondary } : {}) } };
  }
  const placed = atSite.length ? "placed" : "unplaced";
  const rules = codeRules[code] ?? [];
  // A withholding covers the code only at a recorded position (review 4c, R5; codes.mjs `withheldAtStack`): the top authored frame of
  // a stack Chrome recorded for the code (the read, write or throw), or the exact call position of a deeper one that mounts
  // nothing (call-starts.mjs `mount`). Not the site's lines, not a line range; a frame with no column, an unplaced diagnostic and a placeless code
  // place nothing. Harness frames never reach `attributed`; without the program's text no deeper frame counts.
  const source = String(built?.files?.["App.tsx"] ?? built?.text ?? "").split("\n");
  const text = String(built?.files?.["App.tsx"] ?? built?.text ?? "");
  const mount = source.length > 1 ? (frame) => callStarts(text).mount(frame.line, frame.column) : () => true;
  const startsOf = source.length > 1 ? frameStarts(text) : null;
  const stacks = events.map((event) => withStarts(appStack(event.positions), startsOf));
  // A callback's registering argument counts as in classify.mjs (`cb1` in `onSettled(cb1)`; ADR 0815): the two judges agree. For a
  // claiming or declining row only: a withholding there would cover every code raised in that callback at any depth (R5, R25).
  const registeredAt = built?.program && built.lineTable ? registrationPositions(built.program, built.lineTable, text) : () => [];
  const registered = events.flatMap((event) => (attributed(event)[0] ? registeredAt(attributed(event)[0].line) : []));
  const recordedAt = (finding) => stacks.some((stack) => withheldAtStack(finding, stack, mount, code)) || (!withholdingRules.includes(finding.rule) && registered.some((position) => finding.line === position.line && finding.column === position.column));
  const claims = (finding) => coversCodeAt(code, finding, recordedAt(finding));
  // Measured, not counted: a withholding whose start line is a frame's line but no recorded position (a column, a mount line).
  const nearOnly = (finding) => !claims(finding) && withholdingRules.includes(finding.rule) && stacks.some((stack) => stack.some((frame) => frame.line === finding.line));
  const covering = relevant.filter((finding) => inLines(finding, region));
  // The checker anchors a finding at the call that runs the misuse (a closure's call, a helper's call), not always at the misuse
  // line itself. In a one-site program a claiming finding anywhere is this misuse's: it is an agree (or a gap) with `remote` set.
  // Review 4c, R8: for a code with a place, only at a recorded position (`recordedAt`, the call that runs it is a deeper frame's); a
  // claiming finding at another site of the code no longer covers it. A placeless code keeps the claiming rule's finding anywhere.
  const placedCode = !SPEC[code].placeless && !isPlacelessCode(code);
  // Chrome gave this code with a place no position with a column (an unplaced diagnostic, the oracle path's records): the judge cannot
  // place it. Unjudged, counted and listed (`unplaced`), never a pass or a false clean.
  if (placedCode && !stacks.some((stack) => stack.some((frame) => Number.isInteger(frame.column)))) return { outcome: "unplaced", detail: { placed, findings: relevant.map((finding) => `${finding.rule}/${finding.kind}@${finding.line}`) } };
  const claimingViolation = relevant.find((finding) => finding.kind === "violation" && rules.includes(finding.rule) && (placedCode ? recordedAt(finding) : inLines(finding, region)))
    ?? (placedCode ? undefined : relevant.find((finding) => finding.kind === "violation" && rules.includes(finding.rule)));
  const remote = Boolean(claimingViolation) && !inLines(claimingViolation, region);
  const violations = covering.filter((finding) => finding.kind === "violation");
  const stray = relevant.filter((finding) => finding.kind === "violation" && !rules.includes(finding.rule) && !inLines(finding, region));
  const atOthers = checker.findings.filter((finding) => finding.kind === "violation" && elsewhere(finding)).map((finding) => `${finding.rule}@${finding.line}`);
  const detail = { ...(atOthers.length ? { atOthers } : {}), placed, findings: covering.map((finding) => `${finding.rule}/${finding.kind}@${finding.line}`), ...(remote ? { remote: `${claimingViolation.rule}@${claimingViolation.line}` } : {}), ...(stray.length ? { stray: stray.map((finding) => `${finding.rule}@${finding.line}`) } : {}), ...(Object.keys(secondary).length ? { secondary } : {}) };
  // A violation of another rule that the runtime substantiates on these lines (one of that rule's codes reported here: a store read
  // in a body is a strict read too) is a second defect at the site, not a wrong rule; the code is judged as if it were absent.
  const holes = holeLines(built, site, others);
  const atHole = (event) => isPlacelessCode(event.code) && holes.includes(locLine(event));
  const corroborated = (finding) => codesForRule(finding.rule).some((other) => known.some((event) => event.code === other && (attributed(event).some((position) => region.includes(position.line)) || atHole(event))));
  // Counted on its own: a row corroborated only through a placeless code's `loc` (`holeCorroborated`), not a stack position.
  const holeOnly = (finding) => !codesForRule(finding.rule).some((other) => known.some((event) => event.code === other && attributed(event).some((position) => region.includes(position.line))));
  // A violation whose silence the control run measured on its own lines (`runtimeSilent`, pipeline.mjs) has no code that could have fired
  // there: `offSiteVerdicts` reads it as no-counterpart, and it names no rule wrongly here. It excuses nothing else: with no claiming rule the
  // row falls through to a recall gap or a false clean.
  // A claiming rule's own violation off the recorded positions names no rule wrongly (R8): it falls through to a false clean.
  const wrong = violations.filter((finding) => !rules.includes(finding.rule) && !corroborated(finding) && !finding.runtimeSilent && !standsBeside(code, finding, runtime) && !partlySilentRules.includes(finding.rule));
  if (violations.length > wrong.length) detail.corroborated = [...new Set(violations.filter(corroborated).map((finding) => finding.rule))];
  if (violations.some((finding) => corroborated(finding) && holeOnly(finding))) detail.holeCorroborated = true;
  // A claiming violation does not excuse another rule's on these lines: on the `agree` path the second finding is judged the same way
  // (`leaf-owner-forbidden-call` beside `no-await-primitive-in-tracked-scope`, ADR 0641).
  const beside = wrong.filter((finding) => !rules.includes(finding.rule));
  // The claiming rule's own violation away from Chrome's positions: the user sees the warning in the wrong place, both wrong and
  // missing (`misplaced`, a defect of its own; R8).
  const misplaced = placedCode && !claimingViolation ? relevant.find((finding) => finding.kind === "violation" && rules.includes(finding.rule)) : undefined;
  if (misplaced) return { outcome: "misplaced", detail: { ...detail, at: `${misplaced.rule}@${misplaced.line}:${misplaced.column}`, diagnostic: stacks.map((stack) => stack[0] && `${stack[0].line}:${stack[0].column}`).filter(Boolean) } };
  if (claimingViolation) return beside.length ? { outcome: "wrong-rule", detail: { ...detail, beside: true, rules: [...new Set(beside.map((finding) => finding.rule))] } } : { outcome: "agree", detail };
  if (wrong.length) return { outcome: "wrong-rule", detail: { ...detail, rules: [...new Set(wrong.map((finding) => finding.rule))] } };
  // A recall gap is the checker declining this code: an uncertifiable finding of a rule that claims or declines it (or a withholding) at the site.
  // Another rule's uncertifiable finding on the same lines says nothing about this code; it is listed, and the program falls through.
  // `declinedBy` says which mechanism declined (codes.mjs): the claiming rule's own obligation, a withholding rule (SC9012 on the line
  // covers any code and can hide another mechanism's miss), or a declining rule. The saturation and `certified` reads split by it.
  const mechanism = (finding) => ((codeRules[code] ?? []).includes(finding.rule) ? "claiming" : withholdingRules.includes(finding.rule) ? "withholding" : "declining");
  if (covering.some(claims)) return { outcome: "recall-gap", detail: { ...detail, declinedBy: mechanism(covering.find(claims)) } };
  const unrelated = covering.length ? { unrelated: covering.map((finding) => `${finding.rule}/${finding.kind}@${finding.line}`) } : {};
  const claimingElsewhere = relevant.find((finding) => claims(finding));
  if (claimingElsewhere) return { outcome: "recall-gap", detail: { ...detail, ...unrelated, declinedBy: mechanism(claimingElsewhere), remote: `${claimingElsewhere.rule}/${claimingElsewhere.kind}@${claimingElsewhere.line}` } };
  // Certified: the file has no uncertifiable finding at all, so the checker claims it proved the whole file. A false clean there is a
  // fail-closed break (invariant 2); in a file with an uncertifiable finding elsewhere it is a recall gap that only lost its place.
  const certified = !checker.findings.some((finding) => !ignoredRules.includes(finding.rule) && finding.kind === "uncertifiable");
  const near = relevant.find(nearOnly);
  // The named uncertifiables starting in the statement that holds the read: Chrome's top frame on the site's lines (a `wrapper` site's read is
  // on its aux line, the site's line is the call). A code with a place and no such frame has none; a placeless code has the site's line.
  // `atRead` when one starts exactly at the position Chrome reported. What open-rows.mjs checks for a row the owner's stop rule covers (ADR 0815).
  const top = stacks.find((stack) => Number.isInteger(stack[0]?.column) && region.includes(stack[0].line))?.[0];
  const statement = top ? statementAt(text, top.line, top.column) : null;
  const namedU = relevant.filter((finding) => finding.kind === "uncertifiable" && finding.blocker && finding.file === "App.tsx"
    && (statement ? statement.contains(finding.line, finding.column) : isPlacelessCode(code) && finding.line === site.line)).map((finding) => ({ rule: finding.rule, id: finding.id, line: finding.line, column: finding.column, blocker: finding.blocker, ...(stacks.some((stack) => withheldAtStack(finding, stack.slice(0, 1), mount, code)) ? { atRead: true } : {}) }));
  return { outcome: "false-clean", detail: { ...detail, ...unrelated, certified, ...(near ? { withheldNear: `${near.rule}@${near.line}:${near.column}` } : {}), ...(namedU.length ? { namedU } : {}) } };
}

/** The misuse site of a built case: its only `use` site. */
export const siteOf = (built) => built.sites.find((site) => site.primary) ?? built.sites.find((site) => site.code) ?? built.sites[0];
/** The regions of a combination program's other uses (misuse.mjs `buildCombo`): a finding there is about them, not the cell's site. */
export const otherRegions = (built) => built.sites.filter((site) => site.code && !site.primary && built.sites.some((other) => other.primary)).map(regionOf);

// ---------------------------------------------------------------------------------------------------------------------
// The cells

export { fnv };
/** A program's seed depends on the run seed, its cell and its index in the cell, so a cell's programs do not move when others are added. */
export const cellSeed = (base, code, context, index) => programSeed((Number(base) ^ fnv(`${code}/${context}`)) >>> 0, index);

export function cellList({ codes, contexts, recall = false } = {}) {
  const cells = [];
  for (const code of recall ? [...TEMPLATE_CODES, ...RECALL_TEMPLATE_CODES] : TEMPLATE_CODES) {
    if (codes && !codes.includes(code)) continue;
    // `--recall`: the recall contexts are cells of their own (misuse.mjs `recallContexts`), after the code's base cells.
    for (const context of [...SPEC[code].contexts, ...(recall ? recallContextsOf(code) : [])]) if (!contexts || contexts.includes(context)) cells.push({ code, context });
  }
  return cells;
}

const THROWN = new Set(thrownCodes.map(([, code]) => code));
const ASYNC_READ_CODES = ["PENDING_ASYNC_UNTRACKED_READ", "PENDING_ASYNC_FORBIDDEN_SCOPE", "LOADING_ON_OUTSIDE_HOLD", "ASYNC_OUTSIDE_LOADING_BOUNDARY"];
/**
 * `--census` (ADR 0614): every (code, context) no misuse or legal cell lists, in its plain shapes: a context a table added for one code, or a
 * code a table never placed, is measured against every other as soon as it exists. The same judge reads them: the runtime says whether the
 * pair is a misuse, so a census pair needs no cell to fail on what rowProblem flags. A pair that earns a table entry leaves the census by getting one.
 */
// ADR 0893: a stored-error context asks about its reader, with the site's code fixed: the census would cross it with every code.
// `aw-shared-*`: the helper's site past its own await has no owner, so a CONTEXT_NOT_FOUND site there throws NO_OWNER_CONTEXT, and the
// row's cell is not the code at its line. Both of its violations are true (SC4001 at the helper, SC4002 at the memo's read before the
// await); `no-owner-context-aw-shared-*` keeps the shape.
const CENSUS_SKIPS = (code) => new Set([...STORED_CONTEXTS, ...STORED_ASYNC_CONTEXTS, ...(code === "CONTEXT_NOT_FOUND" ? ["aw-shared-after", "aw-shared-before"] : [])]);
export function censusCells({ codes, contexts } = {}) {
  const cellsOf = (spec) => [...spec.contexts, ...(spec.recallContexts ?? []), ...(spec.legal ?? []), ...(spec.recallLegal ?? [])];
  const templates = [...TEMPLATE_CODES, ...RECALL_TEMPLATE_CODES];
  const all = [...new Set(templates.flatMap((code) => cellsOf(SPEC[code])))];
  return templates.filter((code) => !codes || codes.includes(code)).flatMap((code) => {
    const listed = new Set(cellsOf(SPEC[code]));
    // `handler-mixed` runs the site in the body too, and a pending read there halts the mount before the driven button exists: no measure.
    const swallowed = new Set([...(THROWN.has(code) ? PROMISE_SWALLOWS : []), ...(ASYNC_READ_CODES.includes(code) ? ["handler-mixed"] : [])]);
    return all.filter((context) => !listed.has(context) && !swallowed.has(context) && !CENSUS_SKIPS(code).has(context) && (!contexts || contexts.includes(context))).map((context) => ({ code, context }));
  });
}
/** A census pair's shapes: each base variant the code's local setup takes, written directly in the context. */
export const censusShapes = (code) => variantsOf(code).filter((variant) => !SPEC[code].variantSources?.[variant]).map((variant) => ({ variant, binding: "direct", layer: "none", census: true }));

/**
 * `--recall`: the shapes the recall axes add to a cell, each new axis value crossed with the whole base grid (every variant, binding
 * and layer the cell takes, mounted at the root with local values) and not with the other new axes:
 *   bindings  each recall binding of the variant's call kind (misuse.mjs RECALL_API_BINDINGS, RECALL_VALUE_BINDINGS, CALLEE_FORMS),
 *             at every base layer
 *   layers    each recall layer (control flow, and the module helper's import forms), under every base binding
 *   callback  each callback form (CALLBACK_FORMS), on every base shape of a context whose site is in a primitive's callback
 *   mounts    each recall mount (mounts.mjs RECALL_MOUNTS), on every base shape
 *   files     each recall source (SOURCE_AXES.files) a base shape takes
 * A recall context (a cell only `--recall` lists) takes its base grid as well. `base` is the cell's base grid (`gridShapes`, or the
 * legal grid's `legalShapes`); `variants` its variants.
 */
/**
 * The recall grids are lean by default (README "Demoting saturated recall values"; Fable's report 2026-11-03, R0.3): the recall mounts
 * and callback forms whose every program answered as a kept one's on a full run (`redundancy.mjs`, cd08cd408, seed 1): ten mounts as
 * `show-signal`'s program of the same shape (`--against show-signal`), `loading` as the root program, `function` and `let` as `const`
 * (`--against-callback const`). Kept: `show-signal` and `const` (each differs from the base program), `named`, `for-item`, `dynamic`,
 * `show-later`. `--recall-full` brings every value back (the weekly full run, a probe of a demoted value); `--recall-lean` is the
 * default, accepted as a no-op.
 */
export const LEAN_DEMOTED = {
  mounts: ["show-fallback", "show-keyed", "for-signal", "for-unkeyed", "for-fallback", "switch", "repeat", "loading", "loading-on", "errored", "portal"],
  callbacks: ["function", "let"],
};
export function recallAxes(code, context, base, variants, options = {}) {
  const shapes = recallShapesOf(code, context, base, variants, options);
  // ADR 0893: `st-unread` takes every layer around the site, on the direct binding: a layer the walk cannot follow is its question.
  return context === "st-unread" ? shapes.filter((shape) => shape.binding === "direct") : shapes;
}
function recallShapesOf(code, context, base, variants, { legal = false, lean = true } = {}) {
  // A recall-only code has no base grid: its `legal` contexts are recall cells too.
  const recallCell = (legal ? recallLegalOf : recallContextsOf)(code).includes(context) || (legal && SPEC[code].recallOnly && SPEC[code].legal?.includes(context));
  // A variant a recall-only context cannot take (`recallVariantsNotIn`: there it reports another code, so it is neither cell).
  // A legal cell only some legal variants take (misuse.mjs `variantOnlyLegal`: an `ownedWrite` signal's write in a body) takes those.
  const only = legal ? SPEC[code].variantOnlyLegal?.[context] : null;
  const takes = (variant) => !SPEC[code].recallVariantsNotIn?.[context]?.includes(variant) && (!only || only.includes(variant));
  const shapes = recallCell ? base.filter((shape) => takes(shape.variant)) : [];
  // A callback-prop context (misuse.mjs CBP_CONTEXTS) takes its base grid only: its own axis is the pass form (30 contexts a code),
  // and the recall axes, crossed on the base contexts already, would multiply it about twentyfold. So do the lazy-memo, deferred-apply,
  // later-fire, named-handler and never-run contexts (misuse.mjs BASE_ONLY_CONTEXTS): their question is the host's run, not the
  // spelling around the site.
  // ADR 0893: the question of a stored-error context is its reader or its compute.
  if ((STORED_CONTEXTS.includes(context) || STORED_ASYNC_CONTEXTS.includes(context)) && context !== "st-unread") return shapes.filter((shape) => shape.binding === "direct" && shape.layer === "none");
  if (CBP_CONTEXTS.includes(context) || BASE_ONLY_CONTEXTS().includes(context)) return shapes;
  for (const variant of variants.filter(takes)) {
    // A variant one source carries (`variantSources`, the settled async memo) is crossed on that source by the base grid; the recall
    // axes are written on the local source, which it cannot take.
    if (SPEC[code].variantSources?.[variant]) continue;
    const known = new Set(gridBindingsOf(code, variant));
    for (const binding of recallBindingsOf(code, variant).filter((b) => !known.has(b))) for (const layer of LAYERS) shapes.push({ variant, binding, layer });
    // A `catch` swallows a code the runtime only throws (codes.mjs thrownCodes): the try-catch layer never shows it.
    // A legal template of CAUGHT_REACTIVE_READ (`noCatchLayer`): a `catch` around the read is the code's own misuse.
    const layers = THROWN.has(code) || (legal && SPEC[code].noCatchLayer) ? RECALL_LAYERS.filter((layer) => layer !== "try-catch") : RECALL_LAYERS;
    for (const binding of gridBindingsOf(code, variant)) for (const layer of layers) shapes.push({ variant, binding, layer });
  }
  // Variants only the recall grids cross (misuse.mjs `recallVariants`), on the base bindings and layers.
  for (const variant of recallVariantsOf(code, { legal }).filter(variantFits(code, context)).filter(takes)) for (const binding of bindingsOf(code, variant)) for (const layer of LAYERS) shapes.push({ variant, binding, layer });
  const local = base.filter((shape) => !shape.mount && !shape.source && takes(shape.variant));
  if (CALLBACK_CONTEXTS.includes(context)) for (const callback of CALLBACK_FORMS.filter((form) => !lean || !LEAN_DEMOTED.callbacks.includes(form))) for (const shape of local) shapes.push({ ...shape, callback });
  // `dynamic()` renders a component with an unlabelled `untrack` (solid-js rc.17 `dynamicCore`, `solid.dev.js:1444-1527`), so a body's
  // strict-read window never opens there: silent on a stale body read, and on rc.21 a pending one retries and renders (ADR 0668). That
  // pending cell is legal, not misuse: `dyn-fn-body-l` and `dyn-jsx-body-l` in flip-read.mjs hold it.
  const bodyLabelled = NO_WINDOW_MOUNT.has(code) && ["body", "root"].includes(context);
  // A mount a recall-only context's code cannot take (`recallMountsNotIn`: there Solid reports nothing).
  const mountTakes = (mount) => !SPEC[code].recallMountsNotIn?.[context]?.includes(mount);
  for (const mount of RECALL_MOUNTS) if (!(mount === "dynamic" && bodyLabelled) && mountTakes(mount) && !(lean && LEAN_DEMOTED.mounts.includes(mount))) for (const shape of local) shapes.push({ ...shape, mount });
  for (const source of SOURCE_AXES.files) for (const shape of local) if (sourcesOf(code, shape.variant, shape.binding, context).includes(source)) shapes.push({ ...shape, source });
  return shapes;
}

/**
 * `--pairs <sets>` (misuse.mjs PAIR_SETS, default all): the pair grid's shapes of a base cell (a code's base context, or a legal one),
 * each pair of axis values crossed with the base variants, bindings and layers the cell takes (the layer is the pair's own in
 * `handoff-file`). A pair a shape cannot take is left out, as the recall grids leave it: a source the (code, variant, binding) does not
 * take (`sourcesOf`), `dynamic` but for a module hook (it passes no props) and for a body read no window covers (`NO_WINDOW_MOUNT`), a
 * `try-catch`-free code's hand-off is every one. Recall-only contexts take no pairs.
 */
export function pairShapes(code, context, { legal = false, sets = PAIR_SETS } = {}) {
  const spec = SPEC[code];
  // A later mount of an async source (`show-later` × `async-*`): the parent's owned async memo settles before the button mounts the
  // component, so its first read is no pending read (measured on the fast runtime, 2026-11-03: rc.17 reports STRICT_READ_UNTRACKED
  // there, never the pending code). An unowned memo with no reader does not stay settled: it goes dormant and recomputes (ADR 0661). Such a pair of an async template is a legal program in the code's misuse contexts.
  const settled = (mount, source) => LATER_MOUNTS.includes(mount) && source.startsWith("async-") && spec.setup(null, (prefix) => `${prefix}1`).some((sig) => sig.kind === "async");
  const misuseContext = spec.contexts.includes(context);
  if (!(legal ? [...(spec.legal ?? []), ...spec.contexts] : spec.contexts).includes(context)) return [];
  const onlySettled = legal && misuseContext && !(spec.legal ?? []).includes(context);
  const variants = (legal ? legalVariantsOf(code) : gridVariantsOf(code)).filter(variantFits(code, context)).filter((variant) => !spec.variantSources?.[variant]);
  const bodyLabelled = NO_WINDOW_MOUNT.has(code) && ["body", "root"].includes(context);
  const shapes = [];
  for (const variant of variants) for (const binding of gridBindingsOf(code, variant)) {
    if (sets.includes("mount-source")) {
      const takes = sourcesOf(code, variant, binding, context);
      for (const source of PAIR_SOURCES.filter((one) => takes.includes(one))) for (const mount of PAIR_MOUNTS) {
        if (mount === "dynamic" && (source !== "module-hook" || bodyLabelled)) continue;
        // A mount this context's code cannot take (`recallMountsNotIn`: a body re-run onto a fresh pending memo never ends, and reports nothing).
        if (spec.recallMountsNotIn?.[context]?.includes(mount)) continue;
        // A settled pair is no misuse: the legal grid takes it, in the code's misuse contexts (`onlySettled`).
        if (!legal && settled(mount, source)) continue;
        if (onlySettled && !settled(mount, source)) continue;
        for (const layer of LAYERS) shapes.push({ variant, binding, layer, mount, source });
      }
    }
    if (onlySettled) continue;
    if (sets.includes("handoff-file")) for (const layer of PAIR_LAYERS) shapes.push({ variant, binding, layer });
    if (sets.includes("callback-handoff") && CALLBACK_CONTEXTS.includes(context)) for (const callback of PAIR_CALLBACKS) for (const layer of HANDOFF_FORMS) shapes.push({ variant, binding, layer, callback });
  }
  return shapes;
}
/** `--pairs a,b` or `--pairs all`. */
export function pairsOption(value) {
  const sets = !value || value === true || value === "all" ? PAIR_SETS : String(value).split(",");
  for (const set of sets) if (!PAIR_SETS.includes(set)) throw new Error(`--pairs: unknown pair set ${set} (${PAIR_SETS.join(", ")}, all)`);
  return sets;
}
const NO_WINDOW_MOUNT = new Set(["STRICT_READ_UNTRACKED", "PENDING_ASYNC_UNTRACKED_READ"]);
// `contextOnly`: a context only these variants take (its source is written by the variant, so no other value can be crossed with it).
const variantFits = (code, context) => (variant) => (context === undefined || !SPEC[code].variantContexts?.[variant] || SPEC[code].variantContexts[variant].includes(context)) && (SPEC[code].contextOnly?.[context]?.includes(variant) ?? true);
/** Whether a grid's options ask for the lean recall grid: `--recall` without `--recall-full` (`--recall-lean` is the default, a no-op). */
export const recallLeanOf = (options) => Boolean(options.recall) && !options.recallFull;
/** The misuse grid's recall shapes of a cell (`--recall`); `lean: false` is `--recall-full`. */
export const recallShapes = (code, context, { lean = true } = {}) => recallAxes(code, context, gridShapes(code, context), gridVariantsOf(code).filter(variantFits(code, context)), { lean }).filter(fireShape(context));

/**
 * `--grid`: every shape of a cell, one program each. A shape is the variant, the binding and the layer; on the recorded matrices every
 * program of one shape had the same outcome, and no smaller key did, so one program per shape judges what the random draw judges and
 * reaches the shapes twenty random draws miss. In a fixed order: variants, then bindings, then layers.
 */
export const gridShapes = (code, context, mounts = ["root"], sources = ["local"]) => gridVariantsOf(code)
  .filter(variantFits(code, context))
  .flatMap((variant) => gridBindingsOf(code, variant).flatMap((binding) => LAYERS.flatMap((layer) => shapeAxes(code, variant, binding, mounts, sources, context).map((axes) => ({ variant, binding, layer, ...axes })))));

/**
 * The mount and source of a shape's programs: each mount with the local source, then each other source the (code, variant,
 * binding) takes, mounted as the source says (misuse.mjs SOURCES; a source is not crossed with the mounts). `{}` is the root.
 */
export function shapeAxes(code, variant, binding, mounts, sources, context) {
  const takes = sourcesOf(code, variant, binding, context);
  return [
    ...(sources.includes("local") && takes.includes("local") ? mounts.map((mount) => (mount === "root" ? {} : { mount })) : []),
    ...sources.filter((source) => source !== "local" && takes.includes(source)).map((source) => ({ source })),
  ];
}

/** `--mounts a,b` (misuse.mjs MOUNTS), default the entry's own render only. */
export function mountsOption(value) {
  const mounts = (value ?? "root").split(",");
  for (const mount of mounts) if (!MOUNTS.includes(mount)) throw new Error(`--mounts: unknown mount ${mount} (${MOUNTS.join(", ")})`);
  return mounts;
}

/** The id suffix of a shape's mount (none for the entry's own render, so a root program keeps its id). */
export const mountSuffix = (shape) => `${shape.mount ? `-m-${shape.mount}` : ""}${shape.source ? `-s-${shape.source}` : ""}${shape.callback ? `-cb-${shape.callback}` : ""}${shape.combo ? "-c" : ""}`;
/** The seed key's tail of a shape's mount and source (none for a root, local program, so its seed stays). */
export const axesKey = (shape) => `${shape.mount ? `/${shape.mount}` : ""}${shape.source ? `/s:${shape.source}` : ""}${shape.callback ? `/cb:${shape.callback}` : ""}${shape.combo ? "/combo" : ""}`;
/** `--combine`: each shape's program is a combination program (misuse.mjs `buildCombo`) instead of the shape's own. */
export const combined = (shapes, combine) => (combine ? shapes.map((shape) => ({ ...shape, combo: true })) : shapes);

/** A shape's program seed depends on the run seed and the shape only, so a shape's program does not move when the grid grows. */
export const shapeSeed = (base, code, context, shape) => programSeed((Number(base) ^ fnv(`${code}/${context}/${shape.variant ?? "-"}/${shape.binding}/${shape.layer}${axesKey(shape)}`)) >>> 0, 0);

/** A grid program's id, from its cell and shape alone (before it is generated): `--grid-shard` picks by it. */
export const shapeId = ({ code, context }, shape) => `mx-${code.toLowerCase().replaceAll("_", "-")}-${context}-${shape.variant ?? "x"}-${shape.binding}-${shape.layer.replace("+", "-")}${mountSuffix(shape)}`;

export function generateShape(options, { code, context }, shape, index) {
  const seed = shapeSeed(options.seed, code, context, shape);
  const program = generateProgram(seed, { profile: "misuse", misusePlan: { code, context, ...shape } });
  return { program, built: buildCase(program, { id: shapeId({ code, context }, shape) }), index };
}

export function generateCell(options, { code, context }, index) {
  const seed = cellSeed(options.seed, code, context, index);
  const program = generateProgram(seed, { profile: "misuse", misusePlan: { code, context } });
  return { program, built: buildCase(program, { id: `mx-${code.toLowerCase().replaceAll("_", "-")}-${context}-${index}` }) };
}

const emptyTally = () => Object.fromEntries(OUTCOMES.map((outcome) => [outcome, 0]));
const tally = (rows) => rows.reduce((counts, row) => { counts[row.outcome] += 1; return counts; }, emptyTally());

// ---------------------------------------------------------------------------------------------------------------------
// Chrome

/**
 * Chrome's records for evaluated values: the twin, judged against the clean case's findings, which run beside it in the same oracle
 * call. `withClean: false` runs the twin alone (the runtime differential reads no finding). `--chrome-concurrency` sizes the oracle's
 * page pool (default its own: at most 8); a page mostly waits on the driver's pauses, so a larger pool overlaps them.
 */
async function chromeRecords(values, config, options, out, { withClean = true } = {}) {
  const chromeOptions = { out, install: config.install, bin: config.bin, typefacts: config.typefacts, browser: options.browser, concurrency: options.chromeConcurrency ?? options.concurrency, executionCache: browserExecutionCache(options) };
  const cases = values.map((value) => ({ id: value.built.id, dir: value.caseDir, sites: value.built.sites }));
  return runChrome(cases, { ...chromeOptions, withClean });
}

/** Chrome's outcome for a program, and whether it equals the fast path's. */
function judgeChrome(row, record, built) {
  if (!record || record.runtime?.failure || !record.checker?.ok || record.checker.perturbed?.length) return { status: "chrome-inconclusive", why: record?.runtime?.failure?.message ?? record?.checker?.error ?? record?.checker?.perturbed?.[0] ?? "no record" };
  const { outcome, detail } = judge({ code: row.code, site: row.site, typecheck: { ok: true }, checker: record.checker, runtime: record.runtime, others: row.others, built });
  return { status: outcome === row.outcome ? "same" : outcome === "not-fired" ? "not-fired-in-chrome" : "differs", outcome, detail };
}

// ---------------------------------------------------------------------------------------------------------------------
// Shrinking a false-clean representative

async function shrinkRepresentative(row, value, config, options, out) {
  const target = { code: row.code };
  let counter = 0;
  const evaluateAll = async (programs, tag) => {
    const entries = programs.map((program) => { counter += 1; return { program, built: buildCase(program, { id: `${row.id}-${tag}${counter}` }) }; });
    const results = await evaluate(entries, { ...config, casesDir: join(out, "shrink") });
    return entries.map(({ built }) => ({ built, value: results.get(built.id) }));
  };
  const holds = ({ built, value: result }) => {
    const site = siteOf(built);
    if (!site || !result?.checker) return false;
    const verdict = judge({ code: target.code, site, typecheck: result.typecheck, checker: result.checker, runtime: result.runtime, others: otherRegions(built), built });
    return verdict.outcome === "false-clean";
  };
  const judgeBatch = async (programs) => (await evaluateAll(programs, "fc")).map(holds);
  // The control: the program with its misuse replaced by a literal reports neither the code nor a finding.
  const control = async (program) => {
    const twin = legalTwin(program);
    if (twin === null) return true;
    const [{ value: result }] = await evaluateAll([twin], "twin");
    if (!result?.runtime || !result.checker?.ok || !result.typecheck?.ok) return false;
    return !runtimeEvents(result.runtime).some((event) => event.code === target.code);
  };
  const cheap = (program) => [...walk(program)].some(({ node }) => isSite(node));
  const reduced = await shrink(value.program, async (program) => (await judgeBatch([program]))[0], { budget: options.budget, cheap, control, judgeBatch, batchSize: options.shrinkBatch ?? 8 });
  const built = buildCase(reduced.program, { id: `${row.id}-min`, note: `shrunk from misuse matrix program ${row.id} (${reduced.accepted} reductions, ${reduced.attempts} runs), false clean ${target.code}` });
  const dir = writeCase(join(out, "repros", createHash("sha1").update(built.text).digest("hex").slice(0, 10)), built);
  return { dir, size: built.text.split("\n").length, text: built.text, reductions: reduced.accepted, runs: reduced.attempts, program: reduced.program, built };
}

// ---------------------------------------------------------------------------------------------------------------------
// Report

/**
 * Outcomes by binding x layer across every code: one root cause in the checker (a value alias lost, a helper not followed) shows once
 * here, where the cells count it once per code.
 */
export function featureTable(rows) {
  const out = {};
  for (const row of rows) {
    const key = `${row.binding}|${row.layer}`;
    const slot = (out[key] ??= { programs: 0, ...emptyTally(), falseCleanCertified: 0, codes: [] });
    slot.programs += 1;
    slot[row.outcome] += 1;
    if (row.outcome === "false-clean" && row.detail?.certified) slot.falseCleanCertified += 1;
    if (row.outcome === "false-clean" && !slot.codes.includes(row.code)) slot.codes.push(row.code);
  }
  return Object.fromEntries(Object.entries(out).sort(([a], [b]) => (a < b ? -1 : 1)).map(([key, slot]) => [key, { ...slot, codes: slot.codes.sort() }]));
}

/** Builds the recorded matrix (JSON) from the per-program rows. */
export function buildMatrix(rows, { options, header, typedAway }) {
  const cells = {};
  for (const row of rows) {
    const key = `${row.code}|${row.context}`;
    const cell = cells[key] ??= { code: row.code, context: row.context, rows: [] };
    cell.rows.push(row);
  }
  const out = {};
  for (const [key, cell] of Object.entries(cells)) {
    const counts = tally(cell.rows);
    const confirmed = CONFIRMED.reduce((sum, name) => sum + counts[name], 0);
    const by = (field) => Object.fromEntries([...new Set(cell.rows.map((row) => row[field]))].sort().map((value) => [value, tally(cell.rows.filter((row) => row[field] === value))]));
    out[key] = {
      code: cell.code, context: cell.context, programs: cell.rows.length, ...counts, confirmed,
      agreeRemote: cell.rows.filter((row) => row.outcome === "agree" && row.detail?.remote).length,
      falseCleanCertified: cell.rows.filter((row) => row.outcome === "false-clean" && row.detail?.certified).length,
      recallGapBy: Object.fromEntries(["claiming", "withholding", "declining"].map((by) => [by, cell.rows.filter((row) => row.outcome === "recall-gap" && row.detail?.declinedBy === by).length])),
      secondaryCertified: cell.rows.filter((row) => Object.values(row.detail?.secondary ?? {}).includes("false-clean-certified")).length,
      byBinding: by("binding"), byLayer: by("layer"),
      chrome: chromeSummary(cell.rows),
      verdicts: Object.fromEntries(cell.rows.map((row) => [String(row.seed), row.outcome])),
      ...(cell.rows.find((row) => row.minimal) ? { minimal: cell.rows.find((row) => row.minimal).minimal } : {}),
    };
  }
  const total = tally(rows);
  // Violations away from the site, per rule: they need a look (a template's second defect, or a false positive).
  const strays = {};
  for (const row of rows) for (const label of row.detail?.stray ?? []) { const rule = label.split("@")[0]; strays[rule] = [...(strays[rule] ?? []), row.id]; }
  // Second codes Chrome reported at a site that the checker missed or declined, per code and verdict.
  const secondary = {};
  for (const row of rows) for (const [other, verdict] of Object.entries(row.detail?.secondary ?? {})) {
    const entry = (secondary[other] ??= { "false-clean": [], "false-clean-certified": [], "recall-gap": [] });
    entry[verdict].push(row.id);
  }
  return {
    strays: Object.fromEntries(Object.entries(strays).map(([rule, ids]) => [rule, { programs: ids.length, examples: ids.slice(0, 3) }])),
    secondary: Object.fromEntries(Object.entries(secondary).map(([other, entry]) => [other, Object.fromEntries(Object.entries(entry).map(([verdict, ids]) => [verdict, { programs: ids.length, examples: ids.slice(0, 3) }]))])),
    generator: GENERATOR_VERSION, profile: "misuse", seed: options.seed, perCell: options.grid ? null : options.perCell, ...(options.grid ? { sampling: "grid" } : {}), ...(options.pairs ? { pairs: pairsOption(options.pairs) } : {}), ...(options.recall ? { recallLean: recallLeanOf(options) } : {}), checker: header, total,
    confirmed: CONFIRMED.reduce((sum, name) => sum + total[name], 0),
    falseCleanCertified: rows.filter((row) => row.outcome === "false-clean" && row.detail?.certified).length,
    recallGapBy: Object.fromEntries(["claiming", "withholding", "declining"].map((by) => [by, rows.filter((row) => row.outcome === "recall-gap" && row.detail?.declinedBy === by).length])),
    // The violations away from the site, by verdict (`judge`, classify.mjs `offSiteVerdicts`): `unconnected` ones are the next lead.
    offSite: Object.fromEntries(["false-positive", "unconnected", "unexecuted", "unjudged"].map((verdict) => [verdict, rows.flatMap((row) => row.detail?.offSite ?? []).filter((label) => label.split(":")[1].startsWith(verdict)).length])),
    // Per (rule, verdict, reason), what `gate` holds down (ADR 0641): a rise in violations nothing judged is a blind spot growing.
    offSiteByReason: offSiteByReason(rows),
    byFeature: featureTable(rows), typedAway, cells: out,
  };
}

/**
 * How many checker runs had their event-handler proofs withheld by the harness's own entry (checker.mjs `entryVeto`): with any, a
 * handler context's outcomes say nothing about the checker's handler proofs. Empty when none was.
 */
export function entryVetoSummary(values) {
  const vetoed = [...values.values()].map((value) => value?.checker?.entryVeto).filter(Boolean);
  if (!vetoed.length) return {};
  console.error(`WARNING: the harness entry withheld event-handler proofs in ${vetoed.length} checker run(s): ${vetoed[0]}`);
  return { entryVeto: { runs: vetoed.length, premise: vetoed[0] } };
}

function chromeSummary(rows) {
  const checked = rows.filter((row) => row.chrome);
  if (!checked.length) return null;
  const count = (status) => checked.filter((row) => row.chrome.status === status).length;
  return { checked: checked.length, same: count("same"), differs: count("differs"), notFiredInChrome: count("not-fired-in-chrome"), inconclusive: count("chrome-inconclusive") };
}

const pad = (text, width) => String(text).padEnd(width);

/** Markdown: the code x context table (agree/confirmed per cell, with the gap kinds), then the per-code status. */
export function markdown(matrix, statuses) {
  const keys = Object.values(matrix.cells);
  const contexts = [...new Set(keys.map((cell) => cell.context))];
  const codes = [...new Set(keys.map((cell) => cell.code))];
  const lines = [];
  lines.push(`# Misuse recall matrix`, "");
  lines.push(`Generator ${matrix.generator}, seed ${matrix.seed}, ${matrix.sampling === "grid" ? "one program per shape (variant, binding, layer)" : `${matrix.perCell} programs per cell`}. Checker: ${matrix.checker.label ?? "?"}${matrix.checker.bin ? ` (${matrix.checker.bin})` : ""}.`, "");
  lines.push(`Each cell: \`agree/confirmed\`; then \`W\` wrong-rule, \`G\` recall-gap, \`F\` false-clean, \`M\` misplaced, \`U\` unplaced, \`T\` tsc-overlap, \`n\` not-fired, \`t\` typed-away, \`h\` harness, \`u\` unexecuted (a coded throw halted the page before the site ran). \`†\`: Chrome reproduced none of the sampled programs of the cell. Blank: not a context of that code. Only runtime-confirmed programs count (confirmed = the runtime reported the code at the site).`, "");
  lines.push(`| code | ${contexts.join(" | ")} |`, `| --- | ${contexts.map(() => "---").join(" | ")} |`);
  for (const code of codes) {
    const cells = contexts.map((context) => {
      const cell = matrix.cells[`${code}|${context}`];
      if (!cell) return "";
      const extras = [["W", cell["wrong-rule"]], ["P", cell["false-positive"]], ["G", cell["recall-gap"]], ["F", cell["false-clean"]], ["M", cell.misplaced], ["U", cell.unplaced], ["T", cell["tsc-overlap"]], ["n", cell["not-fired"]], ["t", cell["typed-away"]], ["h", cell.harness], ["u", cell.unexecuted]].filter(([, n]) => n).map(([k, n]) => `${k}${n}`).join(" ");
      // A dagger: Chrome did not reproduce any of the sampled programs of this cell (the fast path's evidence stands alone).
      const unconfirmed = cell.chrome && cell.chrome.checked > 0 && cell.chrome.same === 0 ? " †" : "";
      return `${cell.agree}/${cell.confirmed}${extras ? ` ${extras}` : ""}${unconfirmed}`;
    });
    lines.push(`| ${code} | ${cells.join(" | ")} |`);
  }
  const t = matrix.total;
  lines.push("", `Totals: ${Object.entries(t).map(([k, v]) => `${k} ${v}`).join(", ")}; runtime-confirmed ${matrix.confirmed}. False cleans in a certified file (no uncertifiable finding anywhere: a fail-closed break): ${matrix.falseCleanCertified ?? "-"}.`, "");
  if (matrix.byFeature) {
    lines.push("## By binding and layer, across codes", "", "| binding | layer | programs | agree | recall-gap | false-clean | certified false-clean | codes with a false clean |", "| --- | --- | --- | --- | --- | --- | --- | --- |");
    for (const [key, slot] of Object.entries(matrix.byFeature)) {
      const [binding, layer] = key.split("|");
      lines.push(`| ${binding} | ${layer} | ${slot.programs} | ${slot.agree} | ${slot["recall-gap"]} | ${slot["false-clean"]} | ${slot.falseCleanCertified} | ${slot.codes.length} |`);
    }
    lines.push("");
  }
  if (statuses) {
    lines.push(`## Per code`, "", `| code | status | agree | wrong-rule | recall-gap | false-clean | not-fired | why |`, `| --- | --- | --- | --- | --- | --- | --- | --- |`);
    for (const s of statuses) lines.push(`| ${s.code} | ${s.status} | ${s.agree ?? "-"} | ${s.wrongRule ?? "-"} | ${s.recallGap ?? "-"} | ${s.falseClean ?? "-"} | ${s.notFired ?? "-"} | ${s.why ?? ""} |`);
    lines.push("");
  }
  const fc = keys.filter((cell) => cell["false-clean"]);
  if (fc.length) {
    lines.push(`## False-clean cells`, "");
    for (const cell of fc) lines.push(`- ${cell.code} / ${cell.context}: ${cell["false-clean"]} of ${cell.confirmed}${cell.minimal ? `; minimal (${cell.minimal.size} lines) ${cell.minimal.repro}` : ""}`);
    lines.push("");
  }
  const wr = keys.filter((cell) => cell["wrong-rule"]);
  if (wr.length) {
    lines.push(`## Wrong-rule cells`, "");
    for (const cell of wr) lines.push(`- ${cell.code} / ${cell.context}: ${cell["wrong-rule"]} of ${cell.confirmed}`);
    lines.push("");
  }
  const fp = keys.filter((cell) => cell["false-positive"]);
  if (fp.length) {
    lines.push(`## Off-site false-positive cells (a violation away from the site, run with no code of its rule)`, "");
    for (const cell of fp) lines.push(`- ${cell.code} / ${cell.context}: ${cell["false-positive"]} of ${cell.confirmed}`);
    lines.push("");
  }
  return `${lines.join("\n")}\n`;
}

/** The status of each code from its cells. */
export function codeStatuses(matrix) {
  const out = [];
  for (const code of MISUSE_CODES) {
    const spec = SPEC[code];
    if (spec.typedAway) { out.push({ code, status: "typed-away", why: matrix.typedAway?.[code]?.error ?? spec.why }); continue; }
    const cells = Object.values(matrix.cells).filter((cell) => cell.code === code);
    if (!cells.length) { out.push({ code, status: "not-run" }); continue; }
    const sum = (field) => cells.reduce((n, cell) => n + cell[field], 0);
    const agree = sum("agree"); const confirmed = sum("confirmed");
    const wrongRule = sum("wrong-rule"); const recallGap = sum("recall-gap"); const falseClean = sum("false-clean"); const notFired = sum("not-fired");
    const status = !confirmed ? "untriggered" : agree === confirmed ? "proven" : agree ? "gaps" : "no-recall";
    out.push({ code, status, agree: `${agree}/${confirmed}`, wrongRule, recallGap, falseClean, notFired });
  }
  return out;
}

// ---------------------------------------------------------------------------------------------------------------------
// The gate

/** Counts of a run's off-site violations by `rule|verdict|why`. */
export function offSiteByReason(rows) {
  const counts = {};
  for (const row of rows) for (const entry of row.detail?.offSite ?? []) {
    const key = `${String(entry.finding).replace(/@\d+$/, "")}|${entry.verdict}|${entry.why ?? ""}`;
    counts[key] = (counts[key] ?? 0) + 1;
  }
  return counts;
}

/** The keys of `current` that rose above `baseline`; a baseline without the table holds nothing. */
export function offSiteRises(baseline, current) {
  if (!baseline || !current) return [];
  return Object.entries(current).filter(([key, count]) => count > (baseline[key] ?? 0)).map(([key, count]) => ({ key, baseline: baseline[key] ?? 0, count }));
}

/** Outcomes that judge nothing about the checker: the type check rejected the program, or the run failed (the machine, not the program). */
const UNJUDGED = new Set(["typed-away", "harness", "unexecuted"]);

/**
 * Compares a recorded matrix with a baseline for the same seeds. A cell fails when a program that was judged moves out of `agree`
 * (to a judged outcome) or into `false-clean`; a baseline cell missing from the current matrix fails too. Different seeds, per-cell
 * counts or generator versions are refused, and so is a current run in which most programs are unjudged.
 *
 * A baseline `typed-away` or `harness` is a machine-load artifact, not a verdict: a program that moves out of it is reported as
 * `baselineUnjudged` and never as a regression. A program that moves into one of them is reported as `infrastructure` (a warning:
 * the run, not the checker, changed). Neither fails the gate.
 * A baseline cell of a code that is no longer a claimed defect (advisory now) is `retired`: reported, not failed.
 * @returns {{ ok: boolean, refused?: string, regressions: object[], improvements: object[], baselineUnjudged: object[], infrastructure: object[], retired: object[] }}
 */
export function gate(baseline, current) {
  const empty = { regressions: [], improvements: [], baselineUnjudged: [], infrastructure: [] };
  // A pair grid (`--pairs`) holds other programs than the grid it sits beside: compared only with a pair grid of the same sets.
  if (JSON.stringify(baseline.pairs ?? null) !== JSON.stringify(current.pairs ?? null)) return { ok: false, refused: `the matrices differ in pairs (${baseline.pairs ?? "none"} vs ${current.pairs ?? "none"})`, ...empty };
  if (Boolean(baseline.recallLean) !== Boolean(current.recallLean)) return { ok: false, refused: "one matrix is a lean recall grid and the other a full one (--recall-full): they hold other programs", ...empty };
  for (const field of ["seed", "perCell", "generator", "sampling"]) {
    // A matrix recorded before `sampling` existed drew at random.
    if ((field === "sampling" ? baseline[field] ?? "random" : baseline[field]) !== (field === "sampling" ? current[field] ?? "random" : current[field])) return { ok: false, refused: `the matrices differ in ${field} (${baseline[field]} vs ${current[field]}): the gate compares the same programs`, ...empty };
  }
  // A sampled grid (`--grid-sample`) is for iterating, never a gate: the gate compares whole grids.
  for (const [side, matrix] of [["baseline", baseline], ["current", current]]) if (matrix.gridSample) return { ok: false, refused: `the ${side} matrix is a sample (--grid-sample ${matrix.gridSample.fraction}): the gate compares whole grids`, ...empty };
  // A shard (`--grid-shard`) is compared with the same shard of the baseline only: the same programs.
  if (shardKey(baseline) !== shardKey(current)) return { ok: false, refused: `the matrices are different parts of the grid (baseline ${shardKey(baseline)}, current ${shardKey(current)}): the gate compares the same programs`, ...empty };
  const now = current.total ?? {};
  const all = Object.values(now).reduce((sum, n) => sum + n, 0);
  const unjudged = (now["typed-away"] ?? 0) + (now["tsc-overlap"] ?? 0) + (now.harness ?? 0);
  if (all && unjudged / all > TYPE_ERROR_REFUSAL_SHARE) return { ok: false, refused: `${unjudged} of ${all} programs of the current run are typed-away or harness: it judged nothing (the limit is ${TYPE_ERROR_REFUSAL_SHARE * 100}%)`, ...empty };
  const regressions = []; const improvements = []; const baselineUnjudged = []; const infrastructure = []; const retired = []; const newPrograms = [];
  for (const [key, base] of Object.entries(baseline.cells)) {
    const cell = current.cells[key];
    // A code the policy table no longer counts as a defect (it became advisory) is not measured any more: reported, never a regression.
    if (!cell && policyOf(base.code) && policyOf(base.code).disposition !== "defect-claimed") { retired.push({ cell: key, why: `${base.code} is ${policyOf(base.code)?.disposition ?? "unknown"} now, not measured` }); continue; }
    if (!cell) { regressions.push({ cell: key, why: "cell missing from the current matrix" }); continue; }
    // The baseline's programs. A program only the current run has (a grid shape added since) is new, reported, never a regression.
    const seeds = Object.keys(base.verdicts);
    const added = Object.keys(cell.verdicts).filter((seed) => !(seed in base.verdicts));
    if (added.length) newPrograms.push({ cell: key, programs: added.length });
    const list = (keep) => seeds.filter((seed) => keep(base.verdicts[seed], cell.verdicts[seed])).map((seed) => `${seed}: ${base.verdicts[seed] ?? "missing"} -> ${cell.verdicts[seed] ?? "missing"}`);
    const lost = list((was, is) => was === "agree" && is !== "agree" && !UNJUDGED.has(is));
    const gained = list((was, is) => is === "false-clean" && was !== "false-clean" && !UNJUDGED.has(was));
    if (lost.length) regressions.push({ cell: key, why: `agree ${base.agree} -> ${cell.agree}`, programs: lost });
    if (gained.length) regressions.push({ cell: key, why: `false-clean ${base["false-clean"]} -> ${cell["false-clean"]}`, programs: gained });
    // A violation on code tsc rejects (tsc-overlap.mjs) fails from any baseline, a typed-away one included: it is new evidence.
    // A second code missed in a certified file is a false clean of that code (`judge`, `secondary`).
    if ((cell.secondaryCertified ?? 0) > (base.secondaryCertified ?? 0)) regressions.push({ cell: key, why: `certified second-code misses ${base.secondaryCertified ?? 0} -> ${cell.secondaryCertified}` });
    // A program the runtime stopped firing (a template that drifted, a runtime that changed) leaves the measure: from agree that is a
    // lost agree above; from a false clean or a recall gap it would hide the miss, so it fails too.
    const silenced = list((was, is) => is === "not-fired" && !["agree", "not-fired"].includes(was) && !UNJUDGED.has(was));
    if (silenced.length) regressions.push({ cell: key, why: `not-fired ${base["not-fired"] ?? 0} -> ${cell["not-fired"]}`, programs: silenced });
    // A violation off the site that the runtime contradicts (`judge`'s off-site rows) fails from any judged baseline.
    const contradicted = list((was, is) => is === "false-positive" && was !== "false-positive" && !UNJUDGED.has(was));
    if (contradicted.length) regressions.push({ cell: key, why: `false-positive ${base["false-positive"] ?? 0} -> ${cell["false-positive"]}`, programs: contradicted });
    const overlapping = list((was, is) => is === "tsc-overlap" && was !== "tsc-overlap");
    if (overlapping.length) regressions.push({ cell: key, why: `tsc-overlap ${base["tsc-overlap"] ?? 0} -> ${cell["tsc-overlap"]}`, programs: overlapping });
    const fromUnjudged = list((was, is) => UNJUDGED.has(was) && is !== was);
    if (fromUnjudged.length) baselineUnjudged.push({ cell: key, programs: fromUnjudged });
    const intoUnjudged = list((was, is) => UNJUDGED.has(is) && !UNJUDGED.has(was));
    if (intoUnjudged.length) infrastructure.push({ cell: key, programs: intoUnjudged });
    if (cell.agree > base.agree) improvements.push({ cell: key, why: `agree ${base.agree} -> ${cell.agree}` });
    if (cell["false-clean"] < base["false-clean"]) improvements.push({ cell: key, why: `false-clean ${base["false-clean"]} -> ${cell["false-clean"]}` });
  }
  const rises = offSiteRises(baseline.offSiteByReason, current.offSiteByReason);
  // A code missed in a file with no uncertifiable finding is a certified false clean whatever the row's own outcome, and no baseline excuses it.
  const certified = Object.entries(current.secondary ?? {}).filter(([, verdicts]) => verdicts["false-clean-certified"]).map(([code, verdicts]) => ({ code, ...verdicts["false-clean-certified"] }));
  return { ok: regressions.length === 0 && rises.length === 0 && certified.length === 0, regressions, improvements, baselineUnjudged, infrastructure, retired, newPrograms, offSiteRises: rises, certifiedSecondary: certified };
}

function reportGate(result) {
  if (result.refused) { console.error(`gate refused: ${result.refused}`); return 2; }
  for (const item of result.improvements) console.error(`gate: improved ${item.cell}: ${item.why}`);
  for (const item of result.retired ?? []) console.error(`gate: retired ${item.cell}: ${item.why}`);
  if (result.newPrograms?.length) console.error(`gate: ${result.newPrograms.reduce((n, item) => n + item.programs, 0)} program(s) in ${result.newPrograms.length} cell(s) are new since the baseline (not compared)`);
  for (const item of result.baselineUnjudged ?? []) console.error(`gate: baseline unjudged ${item.cell} (not a regression, the baseline was typed-away or harness):\n    ${item.programs.slice(0, 5).join("\n    ")}`);
  for (const item of result.infrastructure ?? []) console.error(`gate: INFRASTRUCTURE WARNING ${item.cell}: ${item.programs.length} program(s) are now typed-away or harness (the run is suspect, not the checker):\n    ${item.programs.slice(0, 5).join("\n    ")}`);
  for (const item of result.regressions) console.error(`gate: REGRESSION ${item.cell}: ${item.why}${item.programs?.length ? `\n    ${item.programs.slice(0, 5).join("\n    ")}` : ""}`);
  console.error(result.ok ? "gate: ok" : `gate: ${result.regressions.length} regression(s)`);
  return result.ok ? 0 : 1;
}

/**
 * The runtime differential over evaluated grid rows: Chrome runs the sampled programs (the same twin the fast path ran) and each
 * record is compared with the fast path's (`runtime-diff.mjs`). Writes `runtime-diff.json` (summary, per-kind counts, the differing
 * programs) and `runtime-diff.jsonl` (one line per compared program). A Chrome record the harness could not read is retried once.
 */
export async function runtimeDifferential(rows, values, perStratum, config, options, out, label) {
  const axes = parseAxes(options.runtimeDiffAxes);
  const picked = sampleStrata(rows, perStratum, (row) => Boolean(values.get(row.id)?.runtime), axes, Number(options.runtimeDiffCap ?? Infinity));
  console.error(`${label}: runtime differential (Happy DOM vs Chrome) on ${picked.length} programs`);
  const started = Date.now();
  const results = [];
  const CHUNK_SIZE = 160;
  for (let at = 0; at < picked.length; at += CHUNK_SIZE) {
    const chunk = picked.slice(at, at + CHUNK_SIZE);
    const records = await chromeRecords(chunk.map((row) => values.get(row.id)), config, options, out, { withClean: false });
    const unreadable = chunk.filter((row) => chromeUnreadable(records.get(row.id)));
    if (unreadable.length) {
      const again = await chromeRecords(unreadable.map((row) => values.get(row.id)), config, options, out, { withClean: false });
      for (const row of unreadable) if (!chromeUnreadable(again.get(row.id))) records.set(row.id, again.get(row.id));
    }
    for (const row of chunk) {
      const record = records.get(row.id);
      const base = { id: row.id, stratum: stratumOf(row, axes), outcome: row.outcome };
      results.push(chromeUnreadable(record)
        ? { ...base, diffs: [], inconclusive: record?.runtime?.failure?.message ?? "no Chrome record" }
        : (() => {
          const diffs = runtimeDiff(values.get(row.id).runtime, record.runtime);
          return diffs.length ? { ...base, diffs, happy: evidenceOf(values.get(row.id).runtime), chrome: evidenceOf(record.runtime) } : { ...base, diffs };
        })());
    }
    console.error(`${label}: runtime differential ${Math.min(at + CHUNK_SIZE, picked.length)}/${picked.length}, ${Math.round((Date.now() - started) / 1000)}s`);
  }
  const judged = results.filter((result) => !result.inconclusive);
  const summary = { ...summarize(judged), inconclusive: results.length - judged.length, perStratum: perStratum === Infinity ? "all" : perStratum, axes };
  writeFileSync(join(out, "runtime-diff.jsonl"), results.map((result) => JSON.stringify(result)).join("\n") + (results.length ? "\n" : ""));
  writeFileSync(join(out, "runtime-diff.json"), `${JSON.stringify({ summary, differing: judged.filter((result) => result.diffs.length) }, null, 2)}\n`);
  console.error(`${label}: runtime differential ${JSON.stringify(summary)}`);
  return summary;
}

/**
 * `--oracle-candidates <N|all>` (audit.mjs): the programs the runtime differential would take from these rows (up to N per stratum of
 * `--runtime-diff-axes`, the rows with a fast record, in the order of `fnv(id)`), written to `oracle-candidates.jsonl` instead of run.
 * A shard's sample of a stratum is the N smallest of its own rows, so the N smallest of every shard's candidates are the whole grid's
 * sample (audit.mjs `pickOracle`): the Chrome work runs once, on the programs a whole run would compare.
 */
export function writeOracleCandidates(rows, values, perStratum, options, out) {
  const axes = parseAxes(options.runtimeDiffAxes);
  const picked = sampleStrata(rows, perStratum, (row) => Boolean(values.get(row.id)?.runtime), axes);
  writeFileSync(join(out, "oracle-candidates.jsonl"), picked.map((row) => JSON.stringify({ id: row.id, stratum: stratumOf(row, axes), rank: fnv(String(row.id)) })).join("\n") + (picked.length ? "\n" : ""));
  return picked.length;
}

// ---------------------------------------------------------------------------------------------------------------------
// Main

const usage = `usage: node scripts/fuzz/matrix.mjs --out <fresh dir> [--seed N] [--per-cell N] [--codes A,B] [--contexts a,b]
  [--checker-bin <solid-checker>] [--typefacts <solid-typefacts>] [--checker-label <commit>] [--tsc <tsrs>] [--all-cores]
  [--confirm-with-chrome [--chrome-sample 3] [--chrome-false-clean 4] [--chrome-other 2] [--chrome-strata] [--browser <chromium>]] [--shrink [--budget N]]
  [--chrome-concurrency N]              Chrome pages at a time (default the oracle's: at most 8)
  [--chrome-strata]                     sample Chrome per stratum (--runtime-diff-axes; default code, context, source, mount, outcome), not per cell
  [--runtime-diff <N|all>]              compare Happy DOM's runtime record with Chrome's on up to N programs per stratum, or all
  [--runtime-diff-axes a,b]             the stratum (default code,context,source,mount,outcome; runtime-diff.mjs STRATUM_AXES)
                                        (runtime-diff.json, runtime-diff.jsonl; the checker plays no part)
  [--grid [--layers a,b]]               one program per shape (variant x binding x layer) of every cell, instead of --per-cell random draws
  [--mounts root,child,show,for]        with --grid, also mount each program as a parent's child (misuse.mjs MOUNTS; default root)
  [--sources local,props,context,module,async]  with --grid, also take the values from a parent's props, a context, helpers.ts or an
                                        async parent (misuse.mjs SOURCES; axis or source names; default local)
  [--recall]                            with --grid, only the recall axes' shapes (matrix.mjs recallAxes): the recall bindings, callee forms,
                                        layers, callback forms, mounts and file sources, each across the base grid, and the recall contexts
  [--recall-full]                       with --recall, every recall mount and callback form; without it the grid is lean (matrix.mjs
                                        LEAN_DEMOTED left out; --recall-lean says so and is the default)
  [--combine]                           with --grid, each shape's program is a combination: the site among one or two legal uses of
                                        other codes and unrelated reactive code, its values from a drawn source (misuse.mjs buildCombo)
  [--scratch <dir>]                     cases/ and chrome-cases/ in <dir> behind links (scratch.mjs; focus.mjs passes the VM's tmpfs)
  [--cache-dir <dir>] [--no-cache]      the runtime cache (type checks and fast-path records; default $TMPDIR/solid-checker-fuzz-cache.noindex)
  [--no-chrome-cache]                   run Chrome's browser half fresh (by default feedback run --execution-cache reuses it; the checker always reruns)
  [--baseline <matrix.json>]            fail when a judged program leaves agree or enters false-clean (same seeds); moves out of or into typed-away/harness are reported, not failed
  node scripts/fuzz/matrix.mjs --gate <baseline.json> <current.json>`;

const CHUNK = 150;

/**
 * A grid's measurement files beside its rows (matrix.mjs and legal.mjs): `costs.jsonl` and `cost.json` (cost.mjs), `profile.json`
 * (perf.mjs `gridProfile`, with the pipeline's busy totals). Nothing reads them back into a row or a verdict.
 * SOLID_CHECKER_FUZZ_PERF=0 writes neither (the control of the rows-unchanged check).
 */
export function writeGridPerf(out, label, rows, values, budget, profile) {
  if (process.env.SOLID_CHECKER_FUZZ_PERF === "0") return;
  const summary = writeCosts(out, rows.map((row) => costLine(row, values.get(row.id))), budget);
  writeFileSync(join(out, "profile.json"), `${JSON.stringify(profile.json({ programs: rows.length, busy: phaseTimings() }), null, 2)}\n`);
  for (const line of costLines(summary, { prefix: label })) console.error(line);
}

export async function runMatrix(options) {
  const out = resolve(options.out);
  if (existsSync(out)) throw new Error(`use a fresh --out path (${out} exists)`);
  requireTypeChecker(resolveTsc(options.tsc));
  mkdirSync(out, { recursive: true });
  // `--scratch <dir>`: cases/ and chrome-cases/ there, behind links at their usual paths (scratch.mjs).
  if (options.scratch) linkScratch(out, resolve(options.scratch));
  const config = withRuntimeCache(attachDaemon(configFrom({ ...options, profile: "misuse" }, out), options), options);
  try {
    return await sweep(options, config, out);
  } finally {
    await config.fast?.close();
  }
}

async function sweep(options, config, out) {
  // The grid's phase clock (perf.mjs) and its budget (cost.mjs): measurement only, beside the rows.
  const profile = gridProfile();
  const budget = budgetOf(options);
  const phaseStart = performance.now();
  const cells = (options.census ? censusCells : cellList)({ codes: options.codes?.split(","), contexts: options.contexts?.split(","), recall: options.recall });
  let entries = [];
  // `--layers a,b` keeps those layers' shapes (their index, and so their programs, as in the whole grid).
  const layerKept = (shape) => !options.layers || options.layers.split(",").includes(shape.layer);
  const mounts = mountsOption(options.mounts);
  const sources = sourcesOption(options.sources);
  // `--recall` (recallAxes): only the shapes the recall axes add; the base grids keep theirs. `--pairs` (pairShapes): the pair grid's.
  const pairs = options.pairs ? pairsOption(options.pairs) : null;
  const shapesOf = (cell) => (options.census ? censusShapes(cell.code) : pairs ? pairShapes(cell.code, cell.context, { sets: pairs }) : options.recall ? recallShapes(cell.code, cell.context, { lean: recallLeanOf(options) }) : gridShapes(cell.code, cell.context, mounts, sources));
  // `--grid-shard i/N` without `--ids` or a sample: a shard generates only its own programs (their ids are known from the shapes),
  // the same programs as generating the whole grid and keeping the shard; `planned` counts the whole grid's.
  const sharding = gridShardOptions(options);
  const sampling = gridSampleOptions(options);
  const early = sharding && options.grid && !options.ids && !sampling ? sharding : null;
  // `--ids <manifest>` (smoke.mjs): the programs it names, as the whole grid generates them; on a grid, picked by id before generation.
  const ids = options.ids && options.grid ? idFilter(options.ids, "matrix", options.seed, { sample: Number(options.idsSample ?? 0), sampleSeed: options.idsSampleSeed ?? 0 }) : null;
  let planned = 0;
  if (options.grid) for (const cell of cells) combined(shapesOf(cell), options.combine).forEach((shape, index) => {
    if (!layerKept(shape)) return;
    planned += 1;
    if (early && shardOf(shapeId(cell, shape), early.count) !== early.index) return;
    if (ids && !ids.keep(shapeId(cell, shape))) return;
    entries.push({ cell, ...generateShape(options, cell, shape, index) });
  });
  else for (const cell of cells) for (let index = 0; index < options.perCell; index += 1) entries.push({ cell, index, ...generateCell(options, cell, index) });
  if (ids) ids.check(entries);
  else if (options.ids) entries = keepIds(entries, options.ids, "matrix", options.seed, { sample: Number(options.idsSample ?? 0), sampleSeed: options.idsSampleSeed ?? 0 });
  // `--grid-sample <fraction>` (sample.mjs): a stratified sample of the grid's programs, for iterating; matrix.json records it.
  let gridSample = null;
  if (sampling) {
    ({ entries, summary: gridSample } = sampleEntries(entries, sampling));
    console.error(`matrix: ${sampleLine(gridSample)}`);
  }
  // `--grid-shard i/N` (shard.mjs): one part of a grid split across processes; matrix.json records it.
  let gridShard = null;
  if (early) gridShard = { index: early.index, count: early.count, kept: entries.length, of: planned };
  else if (sharding) ({ entries, summary: gridShard } = shardEntries(entries, sharding));
  if (gridShard) console.error(`matrix: ${shardLine(gridShard)}`);
  // The programs the grid selected: 0 is a grid with no shapes for these codes (focus.mjs tells it from a run that judged nothing).
  const selected = entries.length;
  profile.add("generate", performance.now() - phaseStart);
  console.error(`matrix: ${cells.length} cells, ${entries.length} programs (${TEMPLATE_CODES.length} codes); evaluating`);
  const rows = []; const values = new Map();
  // The rows judged so far, chunk by chunk (partial.mjs): what a grid process that dies before programs.jsonl leaves for focus.mjs.
  const partial = partialRows(out, { project: (row) => ({ ...row, site: undefined, region: regionOf(row.site) }) });
  const started = Date.now();
  // With every program's runtime cached no Vitest batch runs, so the workers' share of the machine goes to the checker: the same budget.
  const plain = (list) => list.map(({ program, built }) => ({ program, built }));
  const run = config.allCores && runtimeCached(plain(entries), config) ? { ...config, concurrency: config.concurrency + config.workers } : config;
  if (run !== config) console.error(`matrix: every program's runtime is cached; checker concurrency ${run.concurrency} (${config.concurrency} + the ${config.workers} idle workers)`);
  // The executor's own limit too (exec.mjs setExecutorChecks), back to the grid's after the evaluation.
  if (run !== config) await setExecutorChecks(run);
  // Chunk k's fast path runs while chunks k+1 and k+2 are type-checked and checked (preparedAhead): one checker pool and one Vitest batch
  // at a time, as before, but never one waiting on the other, and the checker pool never idle at a chunk's end. Each chunk's records and verdicts are what `evaluate` gives it.
  const chunks = [];
  for (let at = 0; at < entries.length; at += CHUNK) chunks.push(entries.slice(at, at + CHUNK));
  const prepare = (chunk) => { const stage = prepareEntries(plain(chunk), run); stage.catch(() => {}); return stage; };
  const stageOf = preparedAhead(chunks, prepare);
  for (const [number, chunk] of chunks.entries()) {
    const stage = await stageOf(number);
    const results = await finishEntries(stage, run);
    // The checker on the typed-away programs too (tsc-overlap.mjs): a violation on code tsc rejects is judged, never dropped.
    await checkTypedAway(results, run);
    const at = number * CHUNK;
    for (const { cell, index, program, built } of chunk) {
      const value = results.get(built.id);
      const site = siteOf(built);
      const verdict = value?.error ? { outcome: "harness", detail: { why: value.error } } : profile.time("judge", () => judge({ code: cell.code, site, typecheck: value.typecheck, checker: value.checker, runtime: value.runtime, others: otherRegions(built), built, program }));
      const misuse = program.components[0].misuse;
      const others = otherRegions(built);
      rows.push({ id: built.id, seed: program.seed, index, code: cell.code, context: cell.context, variant: misuse.variant, binding: misuse.binding, layer: misuse.layer, ...(misuse.mount && misuse.mount !== "root" ? { mount: misuse.mount } : {}), ...(misuse.source ? { source: misuse.source } : {}), ...(misuse.combo ? { combo: misuse.combo, others } : {}), site, outcome: verdict.outcome, detail: verdict.detail, lines: built.text.split("\n").length });
      values.set(built.id, value);
    }
    partial.append(rows.slice(at));
    console.error(`matrix: ${Math.min(at + CHUNK, entries.length)}/${entries.length} programs, ${Math.round((Date.now() - started) / 1000)}s, ${JSON.stringify(tally(rows))}`);
  }

  if (run !== config) await setExecutorChecks(config);
  profile.add("evaluate", Date.now() - started);
  console.error(`matrix: evaluated in ${Math.round((Date.now() - started) / 1000)}s; busy time ${formatPhaseTimings()}`);
  assertFindingsMapped();
  // A variant typed away on purpose (misuse.mjs TYPED_VARIANTS) is no sign of a broken type checker (tsc-overlap.mjs).
  const rejected = typeRejectedRows(rows);
  const refusal = typeErrorRefusal(rejected.length, rows.length - rows.filter(typedOnPurpose).length, { first: rejected[0]?.detail?.error });
  if (refusal) throw new Error(refusal);
  const broken = harnessRefusal(rows.filter((row) => row.outcome === "harness").length, rows.length, { first: rows.find((row) => row.outcome === "harness")?.detail?.why });
  if (broken) throw new Error(broken);

  // Chrome: a sample of agree programs per cell, every wrong-rule and false-clean one.
  const chromePhase = performance.now();
  if (options.confirmWithChrome) {
    const pick = [];
    const bySample = new Map();
    // Per cell: up to --chrome-sample agree programs, up to --chrome-false-clean false-clean programs (the smallest first: they are the
    // ones the shrinker takes), every wrong-rule program, and up to --chrome-other recall-gap and not-fired programs (a not-fired program
    // Chrome fires is a Happy DOM gap; a recall gap Chrome does not reproduce is a fast-path artefact). Every cell with a false clean or
    // a wrong rule is thereby confirmed in Chrome.
    const other = options.chromeOther ?? 2;
    const limits = { agree: options.chromeSample ?? 3, "false-clean": options.chromeFalseClean ?? 4, "wrong-rule": Infinity, "false-positive": Infinity, "recall-gap": other, "not-fired": other };
    for (const row of [...rows].sort((a, b) => (a.binding === "destructured") - (b.binding === "destructured") || a.lines - b.lines || a.index - b.index)) {
      if (!(row.outcome in limits)) continue;
      // `--chrome-strata`: per stratum (`--runtime-diff-axes`, default code, context, source, mount and outcome), so a source or a
      // mount is sampled on its own.
      const key = options.chromeStrata ? stratumOf(row, parseAxes(options.runtimeDiffAxes)) : `${row.code}|${row.context}|${row.outcome}`;
      const n = bySample.get(key) ?? 0;
      if (n < limits[row.outcome]) { bySample.set(key, n + 1); pick.push(row); }
    }
    console.error(`matrix: Chrome confirmation of ${pick.length} programs`);
    const chromeStarted = Date.now();
    // Chunks only pace the progress line and the retries: the oracle's pool runs a chunk's cases independently, so a larger chunk
    // changes no record, and spends less time waiting on one chunk's slowest case before the next starts.
    const CHROME_CHUNK = 160;
    for (let at = 0; at < pick.length; at += CHROME_CHUNK) {
      const chunk = pick.slice(at, at + CHROME_CHUNK);
      const records = await chromeRecords(chunk.map((row) => values.get(row.id)), config, options, out);
      for (const row of chunk) row.chrome = judgeChrome(row, records.get(row.id), values.get(row.id)?.built);
      // A browser that would not close in time or a clean run that did not report is the machine's, not the program's: once more.
      const retry = chunk.filter((row) => row.chrome.status === "chrome-inconclusive" && /browser close|no clean reference|no record/.test(String(row.chrome.why)));
      if (retry.length) {
        const again = await chromeRecords(retry.map((row) => values.get(row.id)), config, options, out);
        for (const row of retry) { const next = judgeChrome(row, again.get(row.id), values.get(row.id)?.built); if (next.status !== "chrome-inconclusive") row.chrome = { ...next, retried: true }; }
      }
      console.error(`matrix: Chrome ${Math.min(at + CHROME_CHUNK, pick.length)}/${pick.length}, ${Math.round((Date.now() - chromeStarted) / 1000)}s`);
    }
  }

  // The runtime differential (`--runtime-diff <N|all>`): Happy DOM's record against Chrome's, without the checker, on up to N programs
  // per stratum. Beside the matrix, never in it.
  if (options.confirmWithChrome) profile.add("chrome", performance.now() - chromePhase);
  const perStratum = parsePerStratum(options.runtimeDiff);
  if (perStratum) await profile.time("runtimeDiff", () => runtimeDifferential(rows, values, perStratum, config, options, out, "matrix"));
  if (options.oracleCandidates !== undefined) console.error(`matrix: ${writeOracleCandidates(rows, values, parsePerStratum(options.oracleCandidates), options, out)} oracle candidates`);
  const shrinkPhase = performance.now();

  // Shrink one representative per false-clean cell (the smallest program that Chrome did not refute).
  if (options.shrink) {
    const cellsWithFalseClean = new Map();
    // A destructured binding is the commonest cause of a false clean by far; a cell's representative avoids it when the cell has another
    // (then the minimum shows the next cause), and otherwise is the smallest program.
    const smaller = (a, b) => (a.binding === "destructured") !== (b.binding === "destructured") ? b.binding === "destructured" : a.lines < b.lines;
    for (const row of rows.filter((r) => r.outcome === "false-clean" && (!r.chrome || r.chrome.status === "same"))) {
      const key = `${row.code}|${row.context}`;
      const best = cellsWithFalseClean.get(key);
      if (!best || smaller(row, best)) cellsWithFalseClean.set(key, row);
    }
    for (const [key, row] of cellsWithFalseClean) {
      try {
        const minimal = await shrinkRepresentative(row, values.get(row.id), config, options, out);
        let chrome = null;
        if (options.confirmWithChrome) {
          const result = (await evaluate([{ program: minimal.program, built: minimal.built }], { ...config, casesDir: join(out, "shrink") })).get(minimal.built.id);
          const record = (await chromeRecords([result], config, options, out)).get(minimal.built.id);
          chrome = judgeChrome({ code: row.code, site: siteOf(minimal.built), outcome: "false-clean" }, record).status;
        }
        row.minimal = { repro: minimal.dir.replace(`${out}/`, ""), size: minimal.size, reductions: minimal.reductions, runs: minimal.runs, chrome };
        console.error(`matrix: shrunk ${key}: ${minimal.size} lines (${minimal.reductions} reductions)${chrome ? `, Chrome ${chrome}` : ""}`);
      } catch (error) {
        console.error(`matrix: shrink ${key} failed: ${error.message}`);
      }
    }
  }

  if (options.shrink) profile.add("shrink", performance.now() - shrinkPhase);
  const writePhase = performance.now();
  // Typed-away codes: the evidence is the type checker's own error on the one-line misuse.
  const typedAway = {};
  for (const code of MISUSE_CODES.filter((c) => SPEC[c].typedAway)) {
    const dir = materializeForChecker({ files: { "App.tsx": SPEC[code].probe } }, config.install, `typed-away-${code}`);
    const result = await typecheck(dir, { tsc: config.tsc });
    rmSync(dir, { recursive: true, force: true });
    typedAway[code] = { rejected: !result.ok, error: result.errors?.[0] ?? null };
  }

  const header = { label: options.checkerLabel ?? null, bin: options.checkerBin ? "release build" : null };
  const matrix = { ...buildMatrix(rows, { options, header, typedAway }), ...entryVetoSummary(values), ...(gridSample ? { gridSample } : {}), ...(gridShard ? { gridShard } : {}), selected };
  const statuses = codeStatuses(matrix);
  writeFileSync(join(out, "programs.jsonl"), rows.map((row) => JSON.stringify({ ...row, site: undefined, region: regionOf(row.site) })).join("\n") + "\n");
  writeFileSync(join(out, "matrix.json"), `${JSON.stringify({ ...matrix, statuses }, null, 2)}\n`);
  writeFileSync(join(out, "matrix.md"), markdown(matrix, statuses));
  partial.finish();
  if (config.runtimeCache || config.typecheckCache) {
    // Beside the matrix, not in it: a cached run's matrix.json is the uncached run's, byte for byte.
    writeFileSync(join(out, "cache.json"), `${JSON.stringify(cacheStats(config), null, 2)}\n`);
    console.error(`matrix: runtime cache ${JSON.stringify(cacheStats(config))}`);
  }
  profile.add("write", performance.now() - writePhase);
  writeGridPerf(out, "matrix", rows, values, budget, profile);
  console.error(`matrix: wrote ${join(out, "matrix.md")}`);
  if (options.baseline) return reportGate(gate(JSON.parse(readFileSync(resolve(options.baseline), "utf8")), JSON.parse(JSON.stringify(matrix))));
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  guardGridExit("matrix");
  try {
    const argv = process.argv.slice(2);
    if (argv[0] === "--gate") {
      if (argv.length !== 3) throw new Error(`--gate needs <baseline.json> <current.json>\n${usage}`);
      process.exitCode = reportGate(gate(JSON.parse(readFileSync(resolve(argv[1]), "utf8")), JSON.parse(readFileSync(resolve(argv[2]), "utf8"))));
    } else {
      const options = parseArgs(argv, { extraValues: ["budget-ms", "budget-mb", "scratch", "ids", "ids-sample", "ids-sample-seed", "grid-sample", "grid-sample-seed", "grid-sample-min", "grid-sample-codes", "grid-shard", "per-cell", "codes", "contexts", "chrome-sample", "chrome-false-clean", "chrome-other", "layers", "baseline", "checker-label", "cache-dir", "mounts", "sources", "runtime-diff", "runtime-diff-axes", "runtime-diff-cap", "oracle-candidates", "chrome-concurrency", "pairs"], extraFlags: ["census", "grid", "no-chrome-cache", "combine", "chrome-strata", "recall", "recall-lean", "recall-full"] });
      for (const key of ["perCell", "chromeSample", "chromeFalseClean", "chromeOther", "chromeConcurrency"]) if (options[key] !== undefined) options[key] = Number(options[key]);
      options.perCell ??= 20;
      if (options.help || !options.out) { console.error(usage); process.exitCode = options.help ? 0 : 2; }
      else process.exitCode = await runMatrix(options);
    }
  } catch (error) {
    console.error(`matrix: stopped: ${error.message}`);
    process.exitCode = 2;
  }
}
