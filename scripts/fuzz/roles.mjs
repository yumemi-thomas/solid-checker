// The role-log fold (ADR 0533): which arms of the checker's role and owner classifiers a grid's programs reach, and the three
// saturation numbers of a slice. The checker writes one JSON line per role decision when SOLID_CHECKER_IR_ROLE_LOG names a file or a
// directory (rust/crates/solid-reactive-ir/src/role_log.rs); `focus.mjs --role-log` sets it per grid to `<grid dir>.roles.jsonl` and folds
// each grid into `<grid dir>/roles.json` and the run into `<out>/roles.json`. By hand:
//
//   SOLID_CHECKER_IR_ROLE_LOG=<out>.roles.jsonl node scripts/fuzz/matrix.mjs --out <out> ...
//   node scripts/fuzz/roles.mjs [--against <earlier roles.json>] [--out <roles.json>] <grid dir> [<grid dir> ...]
//
// A grid directory's log is `<grid dir>.roles.jsonl`, else `<grid dir>.roles/` (or `--log <file|dir>` for a single grid). The arm inventory is read from the source:
// every `role_arm!("id", ...)` and `arm_id!("id")` under execution_role/ and owners/.
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const IR_SRC = resolve(here, "../../rust/crates/solid-reactive-ir/src");
/** The role-deciding sources whose arms the inventory lists. */
export const ARM_SOURCES = ["execution_role.rs", "execution_role", "owners"];
const ARM = /\b(?:role_arm|arm_id)!\(\s*(default\s+)?"([^"]+)"/g;

/** The tagged arms of the sources under `root`: id -> { default, file }. Throws on a duplicate id or one whose prefix is not its file. */
export function armInventory(root = IR_SRC) {
  const arms = new Map();
  const visit = (path) => {
    if (statSync(path).isDirectory()) { for (const name of readdirSync(path).sort()) visit(join(path, name)); return; }
    if (!path.endsWith(".rs") || basename(path) === "tests.rs") return;
    const stem = basename(path, ".rs");
    for (const match of readFileSync(path, "utf8").matchAll(ARM)) {
      const id = match[2];
      if (!id.startsWith(`${stem}.`)) throw new Error(`arm ${id} in ${relative(root, path)}: an arm id starts with its file's stem (${stem}.)`);
      if (arms.has(id) && arms.get(id).file !== relative(root, path)) throw new Error(`arm ${id} is tagged twice (${arms.get(id).file}, ${relative(root, path)})`);
      if (arms.has(id)) throw new Error(`arm ${id} is tagged twice in ${relative(root, path)}`);
      arms.set(id, { default: Boolean(match[1]), file: relative(root, path) });
    }
  };
  for (const name of ARM_SOURCES) if (existsSync(join(root, name))) visit(join(root, name));
  return arms;
}

/** A log path's program (the checker work directory `.solid-checker-fuzz/<run>/<id>/`) and the file within it. */
export function programOf(path) {
  const match = /\/\.solid-checker-fuzz\/[^/]+\/([^/]+)\/(.+)$/.exec(path ?? "");
  return match ? { program: match[1], file: match[2] } : { program: null, file: path ?? null };
}

/** Every decision line of a log file or directory (`roles-<pid>.jsonl` files), parsed; malformed lines are counted, not thrown. */
export function readRoleLog(path) {
  if (!existsSync(path)) return { lines: [], malformed: 0, missing: true };
  const files = statSync(path).isDirectory() ? readdirSync(path).filter((name) => name.endsWith(".jsonl")).sort().map((name) => join(path, name)) : [path];
  const lines = [];
  let malformed = 0;
  for (const file of files) {
    for (const text of readFileSync(file, "utf8").split("\n")) {
      if (!text) continue;
      try { lines.push(JSON.parse(text)); } catch { malformed += 1; }
    }
  }
  return { lines, malformed, missing: false };
}

/** Outcomes a judged program counts as a finding of the slice: a recall miss or a false positive (either grid). */
export const FINDING_OUTCOMES = ["false-clean", "wrong-rule", "false-positive"];

/**
 * Folds decision lines into per-arm counts. A decision is keyed by program, kind, file and span, so a program the checker ran twice
 * (a confirmation, a twin) counts once. `passed`: decisions that went through the arm; `answered`: decisions whose answer it gave;
 * `unframed`: hits outside any decision (a table precomputation), which count as reached.
 * `silentByDefault`: decisions answered by a default arm (role_log.rs: a non-reporting role for want of a proof) whose answer no rule
 * reports.
 */
export function foldDecisions(lines, inventory) {
  const arms = new Map();
  const slot = (id) => {
    if (!arms.has(id)) arms.set(id, { passed: 0, answered: 0, unframed: 0, programs: new Set(), roles: {}, kinds: {}, silent: 0 });
    return arms.get(id);
  };
  const seen = new Set();
  const silentSites = new Map();
  let decisions = 0;
  let unframed = 0;
  for (const line of lines) {
    if (line.k === "unframed") {
      // An arm a table precomputation reached outside any rule's decision: reached, with no site to key it by.
      unframed += 1;
      for (const id of line.arms ?? []) slot(id).unframed += 1;
      continue;
    }
    const { program, file } = programOf(line.f);
    const key = `${program}\0${line.k}\0${line.rule ?? ""}\0${file}\0${line.s}\0${line.e}`;
    if (seen.has(key)) continue;
    seen.add(key);
    decisions += 1;
    for (const id of line.arms ?? []) {
      const arm = slot(id);
      arm.passed += 1;
      if (program) arm.programs.add(program);
    }
    if (line.arm) {
      const arm = slot(line.arm);
      arm.answered += 1;
      const answer = line.role ?? `ctx ${line.ctx}`;
      arm.roles[answer] = (arm.roles[answer] ?? 0) + 1;
      const kind = line.k === "owner" ? `owner:${line.rule}` : line.k;
      arm.kinds[kind] = (arm.kinds[kind] ?? 0) + 1;
      if (line.silent) arm.silent += 1;
      if (line.silent && (line.default || inventory?.get(line.arm)?.default)) silentSites.set(line.arm, (silentSites.get(line.arm) ?? 0) + 1);
    }
  }
  const byArm = {};
  for (const id of [...arms.keys()].sort()) {
    const arm = arms.get(id);
    byArm[id] = { passed: arm.passed, answered: arm.answered, ...(arm.unframed ? { unframed: arm.unframed } : {}), programs: arm.programs.size, silent: arm.silent, roles: arm.roles, kinds: arm.kinds, ...(inventory?.get(id)?.default ? { default: true } : {}) };
  }
  const hit = Object.keys(byArm).filter((id) => byArm[id].passed > 0 || byArm[id].answered > 0 || byArm[id].unframed > 0);
  return {
    decisions, unframed, arms: byArm, hit,
    neverHit: inventory ? [...inventory.keys()].filter((id) => !hit.includes(id)).sort() : [],
    notInInventory: inventory ? hit.filter((id) => !inventory.has(id)) : [],
    silentByDefault: { decisions: [...silentSites.values()].reduce((a, b) => a + b, 0), byArm: Object.fromEntries([...silentSites].sort()) },
  };
}

/** A grid's findings per 1,000 programs: fast-path candidates (FINDING_OUTCOMES) and those Chrome confirmed (`chrome.status` same). */
export function slicesYield(rows) {
  const candidates = rows.filter((row) => FINDING_OUTCOMES.includes(row.outcome));
  const confirmed = candidates.filter((row) => row.chrome?.status === "same");
  const per = (count) => (rows.length ? Math.round((count * 1000 * 100) / rows.length) / 100 : 0);
  return { programs: rows.length, candidates: candidates.length, confirmed: confirmed.length, candidatesPerThousand: per(candidates.length), confirmedPerThousand: per(confirmed.length) };
}

/**
 * The fold of one or more grids: per-arm counts, never-hit arms, and the saturation numbers (ADR 0533): `yield` (findings per 1,000
 * programs), `growth` (arms this run hit that `against`, an earlier roles.json, did not; null without one) and `silentByDefault`.
 * A slice whose new programs raise neither findings nor arms is saturated for its axis.
 */
export function foldRoles({ lines, rows, inventory, against = null, malformed = 0, sources = [] }) {
  const folded = foldDecisions(lines, inventory);
  const earlier = against ? new Set(against.hit ?? []) : null;
  const newArms = earlier ? folded.hit.filter((id) => !earlier.has(id)) : null;
  return {
    sources,
    armsKnown: inventory ? inventory.size : null,
    armsHit: folded.hit.length,
    decisions: folded.decisions,
    unframed: folded.unframed,
    malformed,
    saturation: {
      yield: slicesYield(rows),
      growth: { against: against ? (against.path ?? true) : null, newArms: newArms?.length ?? null, arms: newArms },
      silentByDefault: folded.silentByDefault.decisions,
    },
    neverHit: folded.neverHit,
    notInInventory: folded.notInInventory,
    silentByDefault: folded.silentByDefault.byArm,
    hit: folded.hit,
    arms: folded.arms,
  };
}

const readRows = (dir) => existsSync(join(dir, "programs.jsonl")) ? readFileSync(join(dir, "programs.jsonl"), "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line)) : [];

/** The log of a grid directory: `<dir>.roles.jsonl` (one file every checker appends to), else `<dir>.roles/` (a file per process). */
export const gridLog = (dir) => (existsSync(`${dir}.roles.jsonl`) || !existsSync(`${dir}.roles`) ? `${dir}.roles.jsonl` : `${dir}.roles`);

/** Folds grid directories (their `programs.jsonl` and logs, `gridLog` or `logs[i]`) into one report. */
export function foldGrids(dirs, { logs = [], inventory = armInventory(), against = null } = {}) {
  const lines = [];
  const rows = [];
  let malformed = 0;
  const sources = [];
  dirs.forEach((dir, index) => {
    const log = logs[index] ?? gridLog(dir);
    const read = readRoleLog(log);
    if (read.missing) throw new Error(`roles.mjs: no role log at ${log} (run the grid with SOLID_CHECKER_IR_ROLE_LOG=${log})`);
    lines.push(...read.lines);
    malformed += read.malformed;
    rows.push(...readRows(dir));
    sources.push({ grid: dir, log, decisions: read.lines.length });
  });
  return foldRoles({ lines, rows, inventory, against, malformed, sources });
}

/** One line for a person: the three numbers and the arm coverage. */
export function summaryLine(report) {
  const { yield: y, growth, silentByDefault } = report.saturation;
  return `roles: ${report.armsHit}/${report.armsKnown ?? "?"} arms hit (${report.neverHit.length} never), ${report.decisions} decisions; yield ${y.confirmedPerThousand} confirmed / ${y.candidatesPerThousand} candidates per 1,000 of ${y.programs} programs; new arms ${growth.newArms ?? "n/a (no --against)"}; silent by default ${silentByDefault}`;
}

function main(argv) {
  const dirs = [];
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (["--against", "--out", "--log"].includes(arg)) {
      if (argv[index + 1] === undefined) throw new Error(`${arg} needs a value`);
      options[arg.slice(2)] = argv[++index];
    } else if (arg.startsWith("--")) throw new Error(`roles.mjs: unknown option ${arg}`);
    else dirs.push(resolve(arg));
  }
  if (!dirs.length) throw new Error("usage: node scripts/fuzz/roles.mjs [--against <roles.json>] [--out <roles.json>] [--log <file|dir>] <grid dir> [...]");
  if (options.log && dirs.length !== 1) throw new Error("roles.mjs: --log names the log of a single grid");
  const against = options.against ? { ...JSON.parse(readFileSync(resolve(options.against), "utf8")), path: resolve(options.against) } : null;
  const report = foldGrids(dirs, { logs: options.log ? [resolve(options.log)] : [], against });
  const out = resolve(options.out ?? join(dirs[0], "roles.json"));
  writeFileSync(out, `${JSON.stringify(report, null, 2)}\n`);
  console.log(`${summaryLine(report)}\nwrote ${out}`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { main(process.argv.slice(2)); } catch (error) { console.error(error.message); process.exitCode = 2; }
}
