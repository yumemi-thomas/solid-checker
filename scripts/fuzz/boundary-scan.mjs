// The boundary scan (ADR 0935): the checker alone, no Chrome, over the cells of the misuse/legal grids, with the role log on. For every
// (code, context, side) it prints the checker's verdict at the site and the tagged arms that answered the decisions at the site's line. Two
// things come out of it:
//
//   arms   which tagged arm answers silent in which cell. `boundary.mjs` lists an arm under the shape whose witness (legal) cell it answers in; the scan
//          is how that claim is measured, not asserted (`boundary.mjs verify`).
//   screen a misuse cell the checker leaves silent (a candidate false clean) and a legal cell it reports (a candidate false positive). Chrome decides
//          (`make fuzz-focus`, `focus.mjs`): the scan is the screen that costs seconds.
//
//   node scripts/fuzz/boundary-scan.mjs --checker-bin <solid-checker-rust> --typefacts <solid-typefacts> [--codes A,B] [--prefix tbl-]
//        [--contexts a,b] [--shapes plain|all|layers] [--ablate] [--concurrency 4] [--out <report.json>] [--install <scripts/fuzz/install>]
//   --shapes layers   the plain shape and, for each recall layer, the direct-binding shape of every variant (the hand-off, depth, chain, control
//                     and file layers cross the contexts in the recall grids; this is how an arm that only a layer reaches is found)
//   --ablate          run each silent arm of a program with SOLID_CHECKER_IR_ABLATE_ARM (the proof-log branch's build): `decisive` lists the
//                     arms whose removal makes a finding appear, the arms the clean stands on
//
// Programs are the grids' own (`generateShape`, `generateLegal`), the checker the fuzzer's own invocation (`runChecker`, SOLID_CHECKER_DAEMON=0).
import { mkdirSync, readFileSync, rmSync, writeFileSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { roleLogLines } from "./roles.mjs";
import { materializeForChecker, runChecker, siteVerdicts } from "./checker.mjs";
import { generateLegal, recallLegalShapes } from "./legal.mjs";
import { generateShape, gridShapes, recallShapes } from "./matrix.mjs";
import { SPEC } from "./misuse.mjs";

const here = dirname(fileURLToPath(import.meta.url));

/** The cells to scan: `{ code, context, side }` for every code, every context of the prefix (or list) on either side of the code's recall lists. */
export function scanCells({ codes, prefix = "", contexts } = {}) {
  const cells = [];
  for (const code of codes) {
    const spec = SPEC[code];
    const lists = [["misuse", [...spec.contexts, ...(spec.recallContexts ?? [])]], ["legal", [...(spec.legal ?? []), ...(spec.recallLegal ?? [])]]];
    for (const [side, names] of lists) {
      for (const context of new Set(names)) {
        if (contexts ? !contexts.includes(context) : !context.startsWith(prefix)) continue;
        cells.push({ code, context, side });
      }
    }
  }
  return cells;
}

/** The shapes of a cell: its plain ones (variant only), or every one the recall grid takes. */
export function scanShapes({ code, context, side, layer, callback, mount }, { shapes = "plain" } = {}) {
  const all = side === "legal" ? recallLegalShapes(code, context, { lean: false }) : recallShapes(code, context, { lean: false });
  const base = all.length ? all : gridShapes(code, context);
  // A callback form with a layer: the callback axis crosses no layer in the grids, so the shape is made from the plain one.
  if (callback && layer && layer !== "none") return gridShapes(code, context).filter((shape) => shape.layer === "none" && shape.binding === "direct" && !shape.mount && !shape.source).map((shape) => ({ ...shape, layer, callback }));
  // A cell that names its callback form or mount takes that axis value's direct-binding shapes (a callback or mount pair of boundary.mjs).
  if (callback) return base.filter((shape) => shape.callback === callback && shape.binding === "direct" && (!shape.layer || shape.layer === "none") && !shape.mount && !shape.source);
  if (mount) return base.filter((shape) => shape.mount === mount && shape.binding === "direct" && (!shape.layer || shape.layer === "none") && !shape.callback && !shape.source);
  // A cell that names its layer takes that layer's direct-binding shapes (a layer pair of boundary.mjs).
  if (layer && layer !== "none") return base.filter((shape) => shape.layer === layer && shape.binding === "direct" && !shape.mount && !shape.source && !shape.callback);
  if (shapes === "all") return base;
  if (shapes === "layers") return base.filter((shape) => shape.binding === "direct" && !shape.mount && !shape.source && !shape.callback);
  return base.filter((shape) => shape.layer === "none" && shape.binding === "direct" && !shape.mount && !shape.source && !shape.callback);
}

/**
 * The decisions of a role log for App.tsx, as `{ kind, line, atSite, role, arm, arms, silent, default }`: the read, write and action decisions
 * (a decision at the site's lines has `atSite`; the others are the helpers and calls the verdict at the site is derived from).
 */
export function decisionsAtLines(logPath, appPath, lines) {
  if (!existsSync(logPath)) return [];
  const text = readFileSync(appPath, "utf8");
  const lineOf = (offset) => text.slice(0, offset).split("\n").length;
  const wanted = new Set(lines);
  const out = [];
  for (const line of roleLogLines(logPath)) {
    if (!["read", "write", "action"].includes(line.k) || !line.f?.endsWith("/App.tsx")) continue;
    const at = lineOf(line.s);
    out.push({ kind: line.k, line: at, atSite: wanted.has(at), role: line.role, arm: line.arm ?? null, arms: line.arms ?? [], silent: line.silent === true, default: line.default === true });
  }
  return out;
}

/** The findings of a run as comparable keys (`id@line:kind`). */
const findingKeys = (result) => new Set((result.findings ?? []).map((finding) => `${finding.id}@${finding.line}:${finding.kind}`));

/** The scan of one program: the checker's verdict at the site and the decisions there. */
export async function scanProgram(item, { bin, typefacts, install, work, ablate = false }) {
  const id = `sc-${item.index}`;
  const dir = materializeForChecker(item.built, install, id);
  const log = join(work, `${id}.roles.jsonl`);
  rmSync(log, { force: true });
  const result = await runChecker(dir, { bin, typefacts, env: { ...process.env, SOLID_CHECKER_IR_ROLE_LOG: log, SOLID_CHECKER_IR_ROLE_LOG_PROGRAM: item.built.id } });
  const site = item.built.sites.find((candidate) => candidate.primary) ?? item.built.sites.find((candidate) => candidate.code) ?? item.built.sites[0];
  const verdict = result.ok ? siteVerdicts([site], result.findings)[0] : null;
  const decisions = result.ok ? decisionsAtLines(log, join(dir, "App.tsx"), [site.line, ...(site.aux ?? [])]) : [];
  // The arms this program's clean stands on: with the ablation hook (the proof-log branch's build), each silent arm that answered anywhere in
  // the program answers Unknown in turn, and the arm is decisive when a finding appears that the baseline did not have.
  const decisive = [];
  if (ablate && result.ok && verdict?.verdict === "none") {
    const base = findingKeys(result);
    for (const arm of new Set(decisions.filter((decision) => decision.silent && decision.arm && !decision.default).map((decision) => decision.arm))) {
      const ablated = await runChecker(dir, { bin, typefacts, env: { ...process.env, SOLID_CHECKER_IR_ABLATE_ARM: arm } });
      if (!ablated.ok) continue;
      const added = [...findingKeys(ablated)].filter((key) => !base.has(key));
      if (added.length) decisive.push({ arm, added });
    }
  }
  rmSync(dir, { recursive: true, force: true });
  rmSync(log, { force: true });
  // Every finding of App.tsx: a rule may stand at the call that runs the function, not at the site's line (`SC1001` at `tf()`, `SC9012` at the
  // registration). The judge reads those as covering the site when the runtime's stack passes through their line.
  const program = (result.findings ?? []).map((finding) => ({ id: finding.id, kind: finding.kind, line: finding.line, blocker: finding.blocker }));
  return { ok: result.ok, error: result.error, verdict: verdict?.verdict ?? null, findings: (verdict?.findings ?? []).map((finding) => ({ id: finding.id, kind: finding.kind, rule: finding.rule, blocker: finding.blocker })), program, decisions, ...(ablate ? { decisive } : {}) };
}

/** The outcome of a scanned cell: `violation`, `uncertifiable` or `clean` at the site. */
export function outcomeOf(scan) {
  if (!scan.ok) return "harness";
  return scan.verdict === "none" ? "clean" : scan.verdict;
}

/** The outcome of the whole program: `clean` only when no finding of App.tsx stands anywhere (a silence nothing accounts for). */
export function programOutcomeOf(scan) {
  if (!scan.ok) return "harness";
  if (scan.program.some((finding) => finding.kind === "violation")) return "violation";
  return scan.program.length ? "uncertifiable" : "clean";
}

/** The programs of cells: `{ ...cell, shape, built, index }` per shape the cell takes (`scanShapes`). */
export function buildItems(cells, { shapes = "plain", start = 0 } = {}) {
  const items = [];
  for (const cell of cells) {
    scanShapes(cell, { shapes }).forEach((shape, index) => {
      const made = cell.side === "legal" ? generateLegal({ seed: 1 }, cell, { index, shape }) : generateShape({ seed: 1 }, cell, shape, index);
      items.push({ ...cell, shape, built: made.built, index: start + items.length });
    });
  }
  return items;
}

/** Runs `run` over `items`, `size` at a time. */
export async function pool(items, size, run) {
  const results = new Array(items.length);
  let next = 0;
  await Promise.all(Array.from({ length: Math.min(size, items.length) }, async () => {
    for (;;) {
      const at = next++;
      if (at >= items.length) return;
      results[at] = await run(items[at], at);
    }
  }));
  return results;
}

async function main(argv) {
  const opt = (name, fallback) => { const at = argv.indexOf(name); return at >= 0 ? argv[at + 1] : fallback; };
  const bin = resolve(opt("--checker-bin", process.env.SOLID_CHECKER_NATIVE_BIN ?? ""));
  const typefacts = resolve(opt("--typefacts", process.env.SOLID_TYPEFACTS_BIN ?? ""));
  const install = resolve(opt("--install", join(here, "install")));
  const codes = opt("--codes", "STRICT_READ_UNTRACKED,REACTIVE_WRITE_IN_OWNED_SCOPE,ACTION_CALLED_IN_OWNED_SCOPE").split(",");
  const cells = scanCells({ codes, prefix: opt("--prefix", ""), contexts: opt("--contexts")?.split(",") });
  const work = resolve(opt("--work", join(here, "../../.solid-checker-fuzz", `boundary-scan-${process.pid}`)));
  mkdirSync(work, { recursive: true });
  const items = buildItems(cells, { shapes: opt("--shapes", "plain") });
  console.log(`boundary scan: ${items.length} program(s) of ${cells.length} cell(s)`);
  const ablate = argv.includes("--ablate");
  const results = await pool(items, Number(opt("--concurrency", 4)), (item) => scanProgram(item, { bin, typefacts, install, work, ablate }));
  const rows = items.map((item, at) => ({ code: item.code, context: item.context, side: item.side, variant: item.shape.variant ?? null, id: item.built.id, outcome: outcomeOf(results[at]), programOutcome: programOutcomeOf(results[at]), ...results[at] }));
  const arms = {};
  for (const row of rows) for (const decision of row.decisions) if (decision.arm) ((arms[decision.arm] ??= {})[`${row.side}:${row.outcome}`] ??= []).push(`${row.code}/${row.context}`);
  const silentMisuse = rows.filter((row) => row.side === "misuse" && row.programOutcome === "clean");
  const reportedLegal = rows.filter((row) => row.side === "legal" && row.programOutcome === "violation");
  console.log(`  misuse cells with no finding anywhere (candidate false cleans, Chrome decides): ${silentMisuse.length}`);
  for (const row of silentMisuse) console.log(`    ${row.id}  arms: ${[...new Set(row.decisions.map((decision) => decision.arm ?? "(untagged)"))].join(", ")}`);
  console.log(`  legal cells with a violation (candidate false positives, Chrome decides): ${reportedLegal.length}`);
  for (const row of reportedLegal) console.log(`    ${row.id}  ${row.program.filter((finding) => finding.kind === "violation").map((finding) => `${finding.id}@${finding.line}`).join(", ")}`);
  const harness = rows.filter((row) => row.outcome === "harness");
  console.log(`  harness failures: ${harness.length}${harness.length ? `, first: ${harness[0].id}: ${String(harness[0].error).slice(0, 200)}` : ""}`);
  const out = opt("--out");
  if (out) writeFileSync(out, `${JSON.stringify({ version: 1, rows, arms }, null, 1)}\n`);
  rmSync(work, { recursive: true, force: true });
  return 0;
}

if (process.argv[1]?.endsWith("boundary-scan.mjs")) process.exit(await main(process.argv.slice(2)));
