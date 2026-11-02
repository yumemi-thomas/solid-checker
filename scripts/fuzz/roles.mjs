// The role-log fold (ADR 0533): which arms of the checker's role and owner classifiers a grid's programs reach, and the three
// saturation numbers of a slice. The checker writes one JSON line per role decision (its file named once per process by a path
// line, `resolvedLine`) when SOLID_CHECKER_IR_ROLE_LOG names a file or a
// directory (rust/crates/solid-reactive-ir/src/role_log.rs); `focus.mjs --role-log` sets it per grid to `<grid dir>.roles.jsonl` and folds
// each grid into `<grid dir>/roles.json` and the run into `<out>/roles.json`. By hand:
//
//   SOLID_CHECKER_IR_ROLE_LOG=<out>.roles.jsonl node scripts/fuzz/matrix.mjs --out <out> ...
//   node scripts/fuzz/roles.mjs [--against <earlier roles.json>] [--out <roles.json>] <grid dir> [<grid dir> ...]
//
// A grid directory's log is `<grid dir>.roles.jsonl`, else `<grid dir>.roles/` (or `--log <file|dir>` for a single grid). The arm inventory is read from the source:
// every `role_arm!("id", ...)` and `arm_id!("id")` under execution_role/ and owners/.
import { closeSync, existsSync, openSync, readdirSync, readFileSync, readSync, statSync, writeFileSync } from "node:fs";
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

/**
 * A log path's program and the file within it. The program is the directory a checker ran in:
 * - the Rust executor's slot, `.solid-checker-fuzz/<run>/slots/<n>/<id>/` (exec/src/main.rs `SlotDir`, the id `%XX`-encoded; a twin's
 *   slot `<n>-run` gives `<id>-run`, the name the JS path gives a twin);
 * - the JS path's work directory, `.solid-checker-fuzz/<run>/<id>/`;
 * - Chrome's staging directory, `.solid-checker-runtime-oracle/<id>/`.
 * A log from before the slots were named (`slots/slot-<n>/`) cannot say which program a decision was: it folds as it always did, every
 * program as the one program `slots` (`legacySlot`, counted as `unattributed` by the fold).
 */
export function programOf(path) {
  const text = path ?? "";
  const slot = /\/\.solid-checker-fuzz\/[^/]+\/slots\/(\d+)(-run)?\/([^/]+)\/(.+)$/.exec(text);
  if (slot) {
    let id;
    try { id = decodeURIComponent(slot[3]); } catch { id = slot[3]; }
    return { program: slot[2] ? `${id}-run` : id, file: slot[4] };
  }
  const work = /\/\.solid-checker-fuzz\/[^/]+\/([^/]+)\/(.+)$/.exec(text);
  if (work) return work[1] === "slots" && /^slot-\d+(?:-run)?\//.test(work[2]) ? { program: "slots", file: work[2], legacySlot: true } : { program: work[1], file: work[2] };
  const oracle = /\/\.solid-checker-runtime-oracle\/([^/]+)\/(.+)$/.exec(text);
  if (oracle) return { program: oracle[1], file: oracle[2] };
  return { program: null, file: path ?? null };
}

/** The files of a log path: the file itself, or a directory's `roles-<pid>.jsonl` files. */
const logFiles = (path) => (statSync(path).isDirectory() ? readdirSync(path).filter((name) => name.endsWith(".jsonl")).sort().map((name) => join(path, name)) : [path]);

/**
 * A log file's lines as the fold reads them, each with its file path in `f`. A checker writes each path once per process, as a
 * `{"k":"path","q":pid,"p":id,"f":path}` line ahead of the lines that name it by `"q"` and `"p"` (role_log.rs); a path line of a pid
 * seen before (a reused pid) replaces the earlier one. A line of an older log carries `f` itself and is read as it is. A line naming a
 * path no earlier line gave is malformed. `paths` is one file's table: processes share a file, never a table across files.
 */
export function resolvedLine(line, paths, counter) {
  if (line.k === "path") {
    paths.set(`${line.q}:${line.p}`, line.f);
    return null;
  }
  if (line.f !== undefined || line.p === undefined) return line;
  const f = paths.get(`${line.q}:${line.p}`);
  if (f === undefined) {
    counter.malformed += 1;
    return null;
  }
  const { q, p, ...rest } = line;
  return { ...rest, f };
}

/**
 * Every decision line of a log path, parsed, read in 8 MiB chunks: a recall grid's log passes V8's string limit (512 MiB, full-7),
 * so no file is held whole. Malformed lines are counted in `counter.malformed`, not thrown. Path lines (`resolvedLine`) are read,
 * not yielded.
 */
export function* roleLogLines(path, counter = { malformed: 0 }) {
  const chunk = Buffer.alloc(8 << 20);
  for (const file of logFiles(path)) {
    const fd = openSync(file, "r");
    const paths = new Map();
    const parsed = (text) => {
      let line;
      try { line = JSON.parse(text); } catch { counter.malformed += 1; return null; }
      return resolvedLine(line, paths, counter);
    };
    let rest = "";
    try {
      for (let read; (read = readSync(fd, chunk, 0, chunk.length, null)) > 0;) {
        const parts = (rest + chunk.toString("utf8", 0, read)).split("\n");
        rest = parts.pop();
        for (const text of parts) {
          if (!text) continue;
          const line = parsed(text);
          if (line) yield line;
        }
      }
      if (rest) {
        const line = parsed(rest);
        if (line) yield line;
      }
    } finally {
      closeSync(fd);
    }
  }
}

/** Every decision line of a log file or directory, parsed into an array (small logs and tests; grids fold `roleLogLines`). */
export function readRoleLog(path) {
  if (!existsSync(path)) return { lines: [], malformed: 0, missing: true };
  const counter = { malformed: 0 };
  const lines = [...roleLogLines(path, counter)];
  return { lines, malformed: counter.malformed, missing: false };
}

/** A decision's key as one number (two FNV-1a halves, 53 bits): millions of keys fit a Set where their strings would not. */
const keyHash = (text) => {
  let a = 0x811c9dc5;
  let b = 0x01000193;
  for (let i = 0; i < text.length; i += 1) {
    const c = text.charCodeAt(i);
    a = Math.imul(a ^ c, 0x01000193) >>> 0;
    b = Math.imul(b ^ c, 0x5bd1e995) >>> 0;
  }
  return (b & 0x1fffff) * 0x100000000 + a;
};

/** Outcomes a judged program counts as a finding of the slice: a recall miss or a false positive (either grid). */
export const FINDING_OUTCOMES = ["false-clean", "wrong-rule", "false-positive"];

/**
 * Folds decision lines into per-arm counts. A decision is keyed by program, kind, file and span, so a program the checker ran twice
 * (a confirmation, a twin) counts once. `passed`: decisions that went through the arm; `answered`: decisions whose answer it gave;
 * `unframed`: hits outside any decision (a table precomputation), which count as reached. `unattributed`: decisions whose path names
 * no single program (an old log's shared slot directory, a path outside any work directory): their dedupe key is shared, so their
 * count is a floor, and they add no program to an arm.
 * `silentByDefault`: decisions answered by a default arm (role_log.rs: a non-reporting role for want of a proof) whose answer no rule
 * reports.
 */
export function foldDecisions(lines, inventory, { perProgram = null } = {}) {
  const arms = new Map();
  const slot = (id) => {
    if (!arms.has(id)) arms.set(id, { passed: 0, answered: 0, unframed: 0, programs: new Set(), roles: {}, kinds: {}, silent: 0 });
    return arms.get(id);
  };
  const seen = new Set();
  const silentSites = new Map();
  let decisions = 0;
  let unframed = 0;
  let unattributed = 0;
  for (const line of lines) {
    if (line.k === "unframed") {
      // An arm a table precomputation reached outside any rule's decision: reached, with no site to key it by.
      unframed += 1;
      for (const id of line.arms ?? []) slot(id).unframed += 1;
      continue;
    }
    const { program: named, file, legacySlot } = programOf(line.f);
    const program = legacySlot ? null : named;
    const key = `${named}\0${line.k}\0${line.rule ?? ""}\0${file}\0${line.s}\0${line.e}`;
    const hashed = keyHash(key);
    if (seen.has(hashed)) continue;
    seen.add(hashed);
    decisions += 1;
    if (!program) unattributed += 1;
    // The cost record's decision count per program (cost.mjs `addDecisions`), when asked for.
    else if (perProgram) perProgram.set(program, (perProgram.get(program) ?? 0) + 1);
    for (const id of line.arms ?? []) {
      const arm = slot(id);
      arm.passed += 1;
      if (program) arm.programs.add(keyHash(program));
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
    decisions, unframed, unattributed, arms: byArm, hit,
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
export function foldRoles({ lines, rows, inventory, against = null, malformed = 0, sources = [], perProgram = null }) {
  const folded = foldDecisions(lines, inventory, { perProgram });
  const earlier = against ? new Set(against.hit ?? []) : null;
  const newArms = earlier ? folded.hit.filter((id) => !earlier.has(id)) : null;
  return {
    sources,
    armsKnown: inventory ? inventory.size : null,
    armsHit: folded.hit.length,
    decisions: folded.decisions,
    unframed: folded.unframed,
    unattributed: folded.unattributed,
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

/**
 * Folds grid directories (their `programs.jsonl` and logs, `gridLog` or `logs[i]`) into one report. `perProgram` (a Map), when given,
 * receives each program's decision count, outside the report (a recall grid's programs would bloat it).
 */
export function foldGrids(dirs, { logs = [], inventory = armInventory(), against = null, perProgram = null } = {}) {
  const rows = [];
  const counter = { malformed: 0 };
  const sources = dirs.map((dir, index) => {
    const log = logs[index] ?? gridLog(dir);
    if (!existsSync(log)) throw new Error(`roles.mjs: no role log at ${log} (run the grid with SOLID_CHECKER_IR_ROLE_LOG=${log})`);
    for (const row of readRows(dir)) rows.push(row);
    return { grid: dir, log, decisions: 0 };
  });
  // The lines are streamed into the fold, never collected: a source's count is the lines read from it.
  function* lines() {
    for (const source of sources) {
      for (const line of roleLogLines(source.log, counter)) {
        source.decisions += 1;
        yield line;
      }
    }
  }
  const report = foldRoles({ lines: lines(), rows, inventory, against, malformed: 0, sources, perProgram });
  report.malformed = counter.malformed;
  return report;
}

/**
 * The run-wide report from the grids' own `roles.json` (`foldGrids` per grid): `--prune-cases` deletes each raw log once its grid is
 * folded, so the run is the sum of the grid reports. Per-arm counts add; a decision is a grid's, never shared between grids, so the sum
 * is the run's; programs add for the same reason. Yield is recomputed from every grid's rows.
 */
export function mergeGridReports(dirs, { inventory = armInventory(), against = null } = {}) {
  const arms = {};
  const rows = [];
  let decisions = 0, unframed = 0, unattributed = 0, malformed = 0;
  const silent = {};
  const sources = [];
  for (const dir of dirs) {
    const report = JSON.parse(readFileSync(join(dir, "roles.json"), "utf8"));
    for (const row of readRows(dir)) rows.push(row);
    decisions += report.decisions ?? 0;
    unframed += report.unframed ?? 0;
    unattributed += report.unattributed ?? 0;
    malformed += report.malformed ?? 0;
    sources.push(...(report.sources ?? []));
    for (const [id, count] of Object.entries(report.silentByDefault ?? {})) silent[id] = (silent[id] ?? 0) + count;
    for (const [id, arm] of Object.entries(report.arms ?? {})) {
      const into = (arms[id] ??= { passed: 0, answered: 0, programs: 0, silent: 0, roles: {}, kinds: {} });
      into.passed += arm.passed ?? 0;
      into.answered += arm.answered ?? 0;
      if (arm.unframed) into.unframed = (into.unframed ?? 0) + arm.unframed;
      into.programs += arm.programs ?? 0;
      into.silent += arm.silent ?? 0;
      for (const [k, v] of Object.entries(arm.roles ?? {})) into.roles[k] = (into.roles[k] ?? 0) + v;
      for (const [k, v] of Object.entries(arm.kinds ?? {})) into.kinds[k] = (into.kinds[k] ?? 0) + v;
      if (arm.default) into.default = true;
    }
  }
  const hit = Object.keys(arms).sort().filter((id) => arms[id].passed > 0 || arms[id].answered > 0 || (arms[id].unframed ?? 0) > 0);
  const earlier = against ? new Set(against.hit ?? []) : null;
  const newArms = earlier ? hit.filter((id) => !earlier.has(id)) : null;
  const silentTotal = Object.values(silent).reduce((x, y) => x + y, 0);
  return {
    sources,
    armsKnown: inventory ? inventory.size : null,
    armsHit: hit.length,
    decisions,
    unframed,
    unattributed,
    malformed,
    saturation: {
      yield: slicesYield(rows),
      growth: { against: against ? (against.path ?? true) : null, newArms: newArms?.length ?? null, arms: newArms },
      silentByDefault: silentTotal,
    },
    neverHit: inventory ? [...inventory.keys()].filter((id) => !hit.includes(id)).sort() : [],
    notInInventory: inventory ? hit.filter((id) => !inventory.has(id)) : [],
    silentByDefault: Object.fromEntries(Object.entries(silent).sort()),
    hit,
    arms: Object.fromEntries(Object.entries(arms).sort()),
  };
}

/** One line for a person: the three numbers and the arm coverage. */
export function summaryLine(report) {
  const { yield: y, growth, silentByDefault } = report.saturation;
  return `roles: ${report.armsHit}/${report.armsKnown ?? "?"} arms hit (${report.neverHit.length} never), ${report.decisions} decisions${report.unattributed ? ` (${report.unattributed} unattributed)` : ""}; yield ${y.confirmedPerThousand} confirmed / ${y.candidatesPerThousand} candidates per 1,000 of ${y.programs} programs; new arms ${growth.newArms ?? "n/a (no --against)"}; silent by default ${silentByDefault}`;
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
