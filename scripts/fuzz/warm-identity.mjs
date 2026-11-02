#!/usr/bin/env node
// The warm checker's identity proof and speed (ADR 0543), on the Rust executor without Chrome: generated programs of one misuse code
// go through step 1 as a grid sends them (the clean and twin projects into slot pairs, their type checks, the checker on the clean
// text), once per pass:
//
//   off     a fresh checker and producer per program (today's grids)
//   on      `--warm-checker`: one retained checker daemon per slot
//   verify  `--warm-verify`: `on`, and every warm answer is followed by a one-shot run at the same path and compared byte for byte
//
// The verify pass is the authority: each program's warm stdout and exit status against the one-shot ones in the same directory. The
// off and on passes are also compared with each other, program by program, with only the slot directory's name normalized (a program
// lands in whichever slot is free). Any difference, any program with no checker answer, or a checker failure exits 1.
//
// Each program goes with its id, as a grid sends it: the off pass names a slot's directory for its program, a warm pass keeps one
// directory per slot (its daemon is keyed by the project path), so the program's directory is normalized too.
//
// `--role-log`: the off and on passes also write the role log (ADR 0533) to `<out>/<pass>.roles.jsonl` (the verify pass would log
// each program twice), and their folds (roles.mjs) must be equal: decisions, arms, programs per arm, malformed lines.
// `--churn`: the programs go in an order where each one's import lines differ from the previous one's wherever the sample allows
// (the warm daemon's slow update; ADR 0543), and `--code` may name several codes, whose programs are interleaved.
//
//   node scripts/fuzz/warm-identity.mjs --code NO_OWNER_CLEANUP --count 3000 --concurrency 8 --out <fresh dir>
//     [--seed 1] [--passes off,on,verify] [--role-log] [--churn] [--checker-bin <release solid-checker>] [--typefacts <solid-typefacts>]
//     [--tsc <tsrs>]
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { checkerEnv, checkerProjectFiles, checkerWorkDir } from "./checker.mjs";
import { DEFAULT_ORACLE_INSTALL } from "./chrome.mjs";
import { createExecutor } from "./exec.mjs";
import { armInventory, foldRoles, roleLogLines } from "./roles.mjs";
import { generateEntries } from "./run.mjs";
import { resolveTsc } from "./typecheck.mjs";

const PASSES = { off: "off", on: "on", verify: "verify" };

export function parseArgs(argv) {
  const options = { seed: 1, count: 1000, concurrency: 8, passes: ["off", "on", "verify"], roleLog: false, churn: false };
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index].replace(/^--/, "").replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
    if (key === "roleLog" || key === "churn") { options[key] = true; continue; }
    const value = argv[index + 1];
    if (value === undefined) throw new Error(`${argv[index]} needs a value`);
    index += 1;
    if (["seed", "count", "concurrency"].includes(key)) options[key] = Number(value);
    else if (key === "passes") options.passes = value.split(",");
    else if (["code", "out", "checkerBin", "typefacts", "tsc", "install"].includes(key)) options[key] = value;
    else throw new Error(`unknown option --${key}`);
  }
  if (!options.code) throw new Error("--code <misuse code> is required");
  if (!options.out) throw new Error("--out <fresh directory> is required");
  for (const pass of options.passes) if (!(pass in PASSES)) throw new Error(`a pass is off, on or verify, not ${pass}`);
  return options;
}

/**
 * `text` with every slot number spelled `*` and the slot's program directory `<program>`: the differences two runs of one program
 * may have (a program lands in whichever slot is free, and its directory is named for it in the off pass, `program` in a warm pass,
 * whose daemon is keyed by the project path). A slot directory is `<root>/<n>/<name>/` or, before the executor named it,
 * `<root>/slot-<n>/`; `-run` marks the twin. The verify pass compares the warm and one-shot answers at the same path, unnormalized.
 */
const escapeRegExp = (text) => text.replace(/[.*+?^${}()|[\]\\]/g, (char) => `\\${char}`);
export const normalizeSlots = (text, root) =>
  text
    .replace(new RegExp(`${escapeRegExp(root)}/(\\d+)(-run)?/[^/"\\\\]+/`, "g"), (_, slot, run = "") => `<slots>/*${run}/<program>/`)
    .replace(new RegExp(`${escapeRegExp(root)}/slot-\\d+(-run)?/`, "g"), (_, run = "") => `<slots>/slot-*${run}/`);

/** The module lines of a program's checked files (imports, re-exports, references): what decides whether a warm update is slow. */
export const moduleLinesOf = (files) =>
  Object.entries(files).filter(([name]) => /\.[cm]?[jt]sx?$/.test(name)).sort(([a], [b]) => (a < b ? -1 : 1))
    .map(([name, text]) => `${name}\n${text.split("\n").filter((line) => /\b(import|from|require|declare)\b|<reference/.test(line)).join("\n")}`).join("\n");

/**
 * `programs` in an order where each one's module lines differ from the previous one's wherever possible (greedy: the next program
 * is the first left whose lines differ, else the first left). Deterministic for one input order.
 */
export function churnOrder(programs, linesOf = (entry) => moduleLinesOf(checkerProjectFiles(entry.built))) {
  const left = programs.map((entry) => ({ entry, lines: linesOf(entry) }));
  const ordered = [];
  let previous = null;
  while (left.length) {
    const index = Math.max(0, left.findIndex(({ lines }) => lines !== previous));
    const [next] = left.splice(index, 1);
    ordered.push(next.entry);
    previous = next.lines;
  }
  return ordered;
}

/** How many consecutive pairs of `programs` have different module lines. */
export const importChanges = (programs) => {
  let changes = 0;
  for (let index = 1; index < programs.length; index += 1) {
    if (moduleLinesOf(checkerProjectFiles(programs[index].built)) !== moduleLinesOf(checkerProjectFiles(programs[index - 1].built))) changes += 1;
  }
  return changes;
};

const quantile = (values, q) => {
  if (!values.length) return null;
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))];
};
const digest = (text) => createHash("sha256").update(text).digest("hex").slice(0, 16);

async function runPass(pass, programs, options, tools) {
  const slotRoot = checkerWorkDir(tools.install, `warm-identity-${pass}`);
  const roleLog = options.roleLog && pass !== "verify" ? join(resolve(options.out), `${pass}.roles.jsonl`) : null;
  const env = Object.fromEntries(["SOLID_CHECKER_RUNTIME_RESOLVER"].filter((key) => process.env[key]).map((key) => [key, process.env[key]]));
  if (roleLog) env.SOLID_CHECKER_IR_ROLE_LOG = roleLog;
  const executor = await createExecutor({
    tsc: tools.tsc, bin: tools.bin, concurrency: options.concurrency, slotRoot, warmChecker: PASSES[pass],
    env: checkerEnv({ typefacts: tools.typefacts, env }),
  });
  const started = process.hrtime.bigint();
  const answers = await Promise.all(programs.map(async ({ built }) => {
    const answer = await executor.prepare({
      id: built.id,
      caseWrites: [],
      project: { files: checkerProjectFiles(built) },
      twinProject: { files: checkerProjectFiles(built, { twin: true }) },
      clean: true, twin: true, checker: { ordinary: false },
    });
    return [built.id, answer];
  }));
  const wallMs = Number(process.hrtime.bigint() - started) / 1e6;
  await executor.close();
  const roles = roleLog ? foldPassLog(roleLog) : null;
  const rows = new Map();
  for (const [id, answer] of answers) {
    const checker = answer.checker;
    rows.set(id, checker
      ? { typecheck: answer.typecheck?.ok, status: checker.status ?? null, spawnError: checker.spawnError, stdout: checker.stdout ?? "", normalized: normalizeSlots(checker.stdout ?? "", slotRoot), wallMs: checker.wallMs, warm: checker.warm, dir: checker.dir }
      : { typecheck: answer.typecheck?.ok, missing: true });
  }
  return { pass, wallMs, rows, slotRoot, roles };
}

/** `value` as JSON with every object's keys sorted. */
export const canonical = (value) => JSON.stringify(value, (_, item) => (item && typeof item === "object" && !Array.isArray(item) ? Object.fromEntries(Object.entries(item).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))) : item));

/** A pass's role log folded (roles.mjs), without the per-source bookkeeping that names the file. */
function foldPassLog(log) {
  const counter = { malformed: 0 };
  const folded = foldRoles({ lines: existsSync(log) ? roleLogLines(log, counter) : [], rows: [], inventory: armInventory() });
  delete folded.sources;
  return { ...folded, malformed: counter.malformed, missing: !existsSync(log) };
}

function summarize({ pass, wallMs, rows }) {
  const checked = [...rows.values()].filter((row) => !row.missing);
  const times = checked.map((row) => row.wallMs);
  const sum = times.reduce((total, value) => total + value, 0);
  return {
    pass, programs: rows.size, checked: checked.length, typeErrors: [...rows.values()].filter((row) => row.typecheck === false).length,
    failures: checked.filter((row) => row.spawnError || ![0, 1].includes(row.status)).length,
    wallS: +(wallMs / 1000).toFixed(2), programsPerS: +(rows.size / (wallMs / 1000)).toFixed(1),
    checkMs: { mean: +(sum / Math.max(1, times.length)).toFixed(1), p50: quantile(times, 0.5), p90: quantile(times, 0.9), p99: quantile(times, 0.99), max: quantile(times, 1), sumS: +(sum / 1000).toFixed(1) },
  };
}

async function main(argv) {
  const options = parseArgs(argv);
  const out = resolve(options.out);
  if (existsSync(out)) throw new Error(`use a fresh --out path (${out} exists)`);
  mkdirSync(out, { recursive: true });
  const tools = {
    bin: options.checkerBin ? resolve(options.checkerBin) : process.env.SOLID_CHECKER_NATIVE_BIN,
    typefacts: options.typefacts ? resolve(options.typefacts) : process.env.SOLID_TYPEFACTS_BIN,
    tsc: resolveTsc(options.tsc), install: resolve(options.install ?? DEFAULT_ORACLE_INSTALL),
  };
  if (!tools.bin || !existsSync(tools.bin)) throw new Error("--checker-bin (or SOLID_CHECKER_NATIVE_BIN) names no checker");
  // Several codes: each gets its share of the count, interleaved program by program, and a seed of its own (a program's id is its
  // seed's, so two codes on one seed would name their programs alike).
  const codes = options.code.split(",");
  const perCode = codes.map((code, index) => generateEntries({ profile: "misuse", code, seed: options.seed + index, count: Math.floor(options.count / codes.length) + (index < options.count % codes.length ? 1 : 0) }));
  const interleaved = [];
  for (let index = 0; index < Math.max(...perCode.map((list) => list.length)); index += 1) for (const list of perCode) if (list[index]) interleaved.push(list[index]);
  const programs = options.churn ? churnOrder(interleaved) : interleaved;
  if (new Set(programs.map(({ built }) => built.id)).size !== programs.length) throw new Error("two programs share an id");
  console.error(JSON.stringify({ programs: programs.length, importChanges: importChanges(programs) }));
  const passes = [];
  for (const pass of options.passes) {
    const result = await runPass(pass, programs, options, tools);
    passes.push(result);
    console.error(JSON.stringify(summarize(result)));
  }
  const problems = [];
  for (const { pass, rows } of passes) {
    for (const [id, row] of rows) {
      if (row.typecheck === false) continue;
      if (row.missing) problems.push({ pass, id, problem: "no checker answer" });
      else if (row.spawnError || ![0, 1].includes(row.status)) problems.push({ pass, id, problem: `checker failed (${row.spawnError ?? `exit ${row.status}`})` });
      else if (pass === "verify" && row.warm?.identical !== true) problems.push({ pass, id, problem: "the warm answer differs from the one-shot answer at the same path", dir: row.dir });
      else if (pass !== "off" && !row.warm) problems.push({ pass, id, problem: "not a warm answer" });
    }
  }
  const verify = passes.find(({ pass }) => pass === "verify");
  const sameRun = verify ? [...verify.rows.values()].filter((row) => row.warm?.identical === true).length : null;
  // Cross-pass: every program's answer, slot names normalized, against the first pass's.
  const crossPass = [];
  const [first, ...rest] = passes;
  for (const other of rest) {
    let equal = 0;
    for (const [id, row] of first.rows) {
      const theirs = other.rows.get(id);
      if (row.missing || theirs?.missing) continue;
      if (row.normalized === theirs.normalized && row.status === theirs.status) equal += 1;
      else problems.push({ pass: other.pass, id, problem: `differs from the ${first.pass} pass (${digest(row.normalized)} vs ${digest(theirs.normalized)})` });
    }
    crossPass.push({ against: first.pass, pass: other.pass, equal });
  }
  // The role log: the off and on passes' folds, equal or a problem.
  const folds = passes.filter(({ roles }) => roles);
  // Key order follows the order the lines were read in (how the processes' appends interleaved), so the folds compare canonically.
  const rolesEqual = folds.length > 1 ? folds.slice(1).every(({ roles }) => canonical(roles) === canonical(folds[0].roles)) : null;
  for (const { pass, roles } of folds) {
    if (roles.missing || roles.decisions === 0) problems.push({ pass, problem: "no role log decisions" });
    if (roles.malformed) problems.push({ pass, problem: `${roles.malformed} malformed role log lines` });
    if (roles.unattributed) problems.push({ pass, problem: `${roles.unattributed} role log decisions name no program` });
  }
  if (rolesEqual === false) problems.push({ problem: `the role log folds differ (${folds.map(({ pass, roles }) => `${pass}: ${roles.decisions} decisions, ${roles.armsHit} arms`).join("; ")})` });
  const roleSummary = folds.length ? Object.fromEntries(folds.map(({ pass, roles }) => [pass, { decisions: roles.decisions, armsHit: roles.armsHit, unframed: roles.unframed, malformed: roles.malformed }])) : null;
  const report = {
    code: options.code, seed: options.seed, count: options.count, concurrency: options.concurrency, checker: tools.bin,
    passes: passes.map(summarize), sameRunIdentical: sameRun, crossPass, roles: roleSummary, rolesEqual, problems: problems.slice(0, 200), problemCount: problems.length,
    programs: Object.fromEntries([...first.rows].map(([id, row]) => [id, row.missing ? null : digest(row.normalized)])),
  };
  writeFileSync(join(out, "warm-identity.json"), `${JSON.stringify(report, null, 2)}\n`);
  if (folds.length > 1 && rolesEqual === false) for (const { pass, roles } of folds) writeFileSync(join(out, `${pass}.roles.json`), `${JSON.stringify(roles, null, 2)}\n`);
  console.error(JSON.stringify({ sameRunIdentical: sameRun, crossPass, roles: roleSummary, rolesEqual, problemCount: problems.length }));
  return problems.length ? 1 : 0;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main(process.argv.slice(2)).then((code) => process.exit(code), (error) => { console.error(error.stack ?? String(error)); process.exit(2); });
}
