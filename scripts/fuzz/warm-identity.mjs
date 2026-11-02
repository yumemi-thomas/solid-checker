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
//   node scripts/fuzz/warm-identity.mjs --code NO_OWNER_CLEANUP --count 3000 --concurrency 8 --out <fresh dir>
//     [--seed 1] [--passes off,on,verify] [--checker-bin <release solid-checker>] [--typefacts <solid-typefacts>] [--tsc <tsrs>]
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { checkerEnv, checkerProjectFiles, checkerWorkDir } from "./checker.mjs";
import { DEFAULT_ORACLE_INSTALL } from "./chrome.mjs";
import { createExecutor } from "./exec.mjs";
import { generateEntries } from "./run.mjs";
import { resolveTsc } from "./typecheck.mjs";

const PASSES = { off: "off", on: "on", verify: "verify" };

export function parseArgs(argv) {
  const options = { seed: 1, count: 1000, concurrency: 8, passes: ["off", "on", "verify"] };
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index].replace(/^--/, "").replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
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
 * `text` with every slot number spelled `*`: the one difference two runs of one program in two slots may have. A slot directory is
 * `<root>/<n>/<program>/` (the executor names it for its program, so the role log keys decisions by program) or, before that,
 * `<root>/slot-<n>/`; `-run` marks the twin.
 */
const escapeRegExp = (text) => text.replace(/[.*+?^${}()|[\]\\]/g, (char) => `\\${char}`);
export const normalizeSlots = (text, root) =>
  text.replace(new RegExp(`${escapeRegExp(root)}/(slot-)?\\d+(-run)?/`, "g"), (_, prefix = "", run = "") => `<slots>/${prefix}*${run}/`);

const quantile = (values, q) => {
  if (!values.length) return null;
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))];
};
const digest = (text) => createHash("sha256").update(text).digest("hex").slice(0, 16);

async function runPass(pass, programs, options, tools) {
  const slotRoot = checkerWorkDir(tools.install, `warm-identity-${pass}`);
  const executor = await createExecutor({
    tsc: tools.tsc, bin: tools.bin, concurrency: options.concurrency, slotRoot, warmChecker: PASSES[pass],
    env: checkerEnv({ typefacts: tools.typefacts, env: Object.fromEntries(["SOLID_CHECKER_RUNTIME_RESOLVER"].filter((key) => process.env[key]).map((key) => [key, process.env[key]])) }),
  });
  const started = process.hrtime.bigint();
  const answers = await Promise.all(programs.map(async ({ built }) => {
    const answer = await executor.prepare({
      caseWrites: [],
      project: { files: checkerProjectFiles(built) },
      twinProject: { files: checkerProjectFiles(built, { twin: true }) },
      clean: true, twin: true, checker: { ordinary: false },
    });
    return [built.id, answer];
  }));
  const wallMs = Number(process.hrtime.bigint() - started) / 1e6;
  await executor.close();
  const rows = new Map();
  for (const [id, answer] of answers) {
    const checker = answer.checker;
    rows.set(id, checker
      ? { typecheck: answer.typecheck?.ok, status: checker.status ?? null, spawnError: checker.spawnError, stdout: checker.stdout ?? "", normalized: normalizeSlots(checker.stdout ?? "", slotRoot), wallMs: checker.wallMs, warm: checker.warm, dir: checker.dir }
      : { typecheck: answer.typecheck?.ok, missing: true });
  }
  return { pass, wallMs, rows, slotRoot };
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
  const programs = generateEntries({ profile: "misuse", code: options.code, seed: options.seed, count: options.count });
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
  const report = {
    code: options.code, seed: options.seed, count: options.count, concurrency: options.concurrency, checker: tools.bin,
    passes: passes.map(summarize), sameRunIdentical: sameRun, crossPass, problems: problems.slice(0, 200), problemCount: problems.length,
    programs: Object.fromEntries([...first.rows].map(([id, row]) => [id, row.missing ? null : digest(row.normalized)])),
  };
  writeFileSync(join(out, "warm-identity.json"), `${JSON.stringify(report, null, 2)}\n`);
  console.error(JSON.stringify({ sameRunIdentical: sameRun, crossPass, problemCount: problems.length }));
  return problems.length ? 1 : 0;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main(process.argv.slice(2)).then((code) => process.exit(code), (error) => { console.error(error.stack ?? String(error)); process.exit(2); });
}
