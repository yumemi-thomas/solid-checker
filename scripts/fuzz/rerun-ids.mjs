#!/usr/bin/env node
// Reruns a finished run's programs of some outcomes (default `harness`) by id: a run with --prune-cases kept no case directory to
// replay, so the programs are generated again from their grid (every program is a pure function of the grid's seed and its shape) and
// judged with Chrome. Writes one manifest per (grid, code) (smoke.mjs `keepIds`: `{ generator, seed, ids }`) and prints the command for
// each, restricted to the codes and contexts the ids name so only their cells are generated.
//
//   node scripts/fuzz/rerun-ids.mjs <run dir> --out <manifest dir> [--outcomes harness,…] [--grids mx-recall,…] [--rerun-out <dir>]
//     [-- <extra grid args, e.g. --checker-bin … --typefacts …>]
// A run dir as focus.mjs writes it: `<grid>/programs.jsonl` (with its matrix.json or legal.json), or `<grid>/<code>/…` with --split-codes.
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { GRIDS } from "./focus.mjs";
import { GENERATOR_VERSION } from "./generate.mjs";

/** The units of a grid directory: itself, or one per code with --split-codes; each with its rows, its grid's seed and whether it was a lean recall grid (`recallLean`). */
export function gridUnits(dir) {
  const units = existsSync(join(dir, "programs.jsonl")) ? [dir] : existsSync(dir) ? readdirSync(dir).map((name) => join(dir, name)).filter((path) => existsSync(join(path, "programs.jsonl"))) : [];
  return units.map((unit) => {
    const report = ["matrix.json", "legal.json"].map((name) => join(unit, name)).find(existsSync);
    const recorded = report ? JSON.parse(readFileSync(report, "utf8")) : {};
    const seed = recorded.seed;
    return { unit, seed, recallLean: recorded.recallLean === true, rows: readFileSync(join(unit, "programs.jsonl"), "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line)) };
  });
}

/** Per (grid, code): the ids of the rows with one of `outcomes`, their contexts, and the grid's seed. */
export function selectRerun(run, { outcomes = ["harness"], grids = Object.keys(GRIDS) } = {}) {
  const out = [];
  for (const grid of grids) for (const { seed, recallLean, rows } of gridUnits(join(run, grid))) {
    const byCode = new Map();
    for (const row of rows.filter((r) => outcomes.includes(r.outcome))) {
      const entry = byCode.get(row.code) ?? { grid, code: row.code, seed, ...(grid.endsWith("-recall") && !recallLean ? { recallFull: true } : {}), ids: [], contexts: new Set(), why: [] };
      entry.ids.push(row.id); entry.contexts.add(row.context); entry.why.push(`${row.id}: ${row.outcome} ${JSON.stringify(row.detail ?? {}).slice(0, 160)}`);
      byCode.set(row.code, entry);
    }
    out.push(...byCode.values());
  }
  return out.map((entry) => ({ ...entry, contexts: [...entry.contexts].sort() }));
}

/**
 * The command that generates a manifest's programs from its grid and judges them, Chrome included. A recall grid whose report did not
 * say it was lean (a run from before the lean default, or `--recall-full`) is rerun with `--recall-full`: its ids may name a demoted value.
 */
export function rerunCommand(entry, manifest, rerunOut, extra = []) {
  const [script, ...args] = GRIDS[entry.grid];
  return ["node", `scripts/fuzz/${script}`, "--grid", ...args, "--seed", String(entry.seed ?? 1), "--codes", entry.code, "--contexts", entry.contexts.join(","), "--ids", manifest,
    ...(entry.recallFull ? ["--recall-full"] : []), "--out", join(rerunOut, `${entry.grid}-${entry.code}`), "--confirm-with-chrome", ...extra].join(" ");
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const argv = process.argv.slice(2);
  const split = argv.indexOf("--");
  const own = split < 0 ? argv : argv.slice(0, split);
  const extra = split < 0 ? [] : argv.slice(split + 1);
  const value = (name) => { const at = own.indexOf(name); return at < 0 ? undefined : own[at + 1]; };
  const run = own[0];
  const out = value("--out");
  if (!run || !out) { console.error("usage: node scripts/fuzz/rerun-ids.mjs <run dir> --out <manifest dir> [--outcomes harness] [--grids a,b] [--rerun-out <dir>] [-- <grid args>]"); process.exit(2); }
  mkdirSync(out, { recursive: true });
  const entries = selectRerun(resolve(run), { outcomes: (value("--outcomes") ?? "harness").split(","), grids: value("--grids")?.split(",") });
  const rerunOut = value("--rerun-out") ?? join(resolve(out), "rerun");
  const commands = [];
  for (const entry of entries) {
    const manifest = join(resolve(out), `${entry.grid}-${entry.code}.json`);
    writeFileSync(manifest, `${JSON.stringify({ generator: GENERATOR_VERSION, seed: String(entry.seed ?? 1), ids: entry.ids, why: entry.why })}\n`);
    commands.push(rerunCommand(entry, manifest, rerunOut, extra));
    console.error(`${entry.grid} ${entry.code}: ${entry.ids.length} program(s) in ${entry.contexts.join(",")}`);
  }
  writeFileSync(join(resolve(out), "rerun.sh"), `#!/usr/bin/env bash\nset -u\n${commands.join("\n")}\n`);
  console.log(commands.join("\n"));
}
