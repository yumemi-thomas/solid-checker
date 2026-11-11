#!/usr/bin/env node
// The package-site cells (ADR 0957) as one command: the programs of the recall grids whose site is in a package (the `-pk-` ids), one `--ids` manifest and one focus run per
// (grid, code), so a fix is checked against them in minutes instead of a whole recall grid. A grid refuses a manifest naming a program it does not generate, so each unit takes its own.
//
//   node scripts/fuzz/package-cells.mjs --list [--codes A,B] [--grids mx,lg] [--entries call,tagged-own]
//   node scripts/fuzz/package-cells.mjs --out <fresh dir> --checker-bin <solid-checker-rust> --typefacts <solid-typefacts> [--codes A,B] [--grids mx,lg] [--entries …] [-- <focus args>]
// `--out` must not exist (focus.mjs and the grids refuse a used directory); every unit's run lands in `<out>/<grid>-<code>`, its manifest in `<out>/manifests`. The exit status is the
// worst of the units' (focus.mjs exits 1 when a run needs a look). Default focus arguments: `--runtime-diff all --seed 1`, and after `--`: `--concurrency`, `--workers`, `--no-chrome-cache`.
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { GENERATOR_VERSION } from "./generate.mjs";
import { legalCells, legalId, recallLegalShapes } from "./legal.mjs";
import { PACKAGE_SITE_CODES, cellList, recallShapes, shapeId } from "./matrix.mjs";

const here = dirname(fileURLToPath(import.meta.url));

/** `[{ grid, code, ids, rows, contexts }]`: the package-site programs of each (grid, code) of the recall grids, in the grids' order (`rows`: each id with its context, variant and `pkg`). */
export function packageCells({ codes = PACKAGE_SITE_CODES, grids = ["mx", "lg"], entries, contexts } = {}) {
  const units = [];
  for (const code of codes) for (const grid of grids) {
    const ids = [];
    const rows = [];
    const used = new Set();
    for (const cell of grid === "mx" ? cellList({ codes: [code], recall: true }) : legalCells({ codes: [code], recall: true })) {
      if (contexts && !contexts.includes(cell.context)) continue;
      const shapes = grid === "mx" ? recallShapes(code, cell.context, { lean: true }) : recallLegalShapes(code, cell.context, { lean: true });
      for (const shape of shapes.filter((one) => one.pkg && (!entries || entries.includes(one.pkg)))) {
        const id = grid === "mx" ? shapeId(cell, shape) : legalId(cell, { shape });
        ids.push(id);
        rows.push({ id, context: cell.context, variant: shape.variant ?? null, pkg: shape.pkg });
        used.add(cell.context);
      }
    }
    if (ids.length) units.push({ grid: `${grid}-recall`, code, ids, rows, contexts: [...used] });
  }
  return units;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const argv = process.argv.slice(2);
  const split = argv.indexOf("--");
  const own = split < 0 ? argv : argv.slice(0, split);
  const passed = split < 0 ? ["--concurrency", "12", "--workers", "12", "--chrome-concurrency", "12", "--no-chrome-cache"] : argv.slice(split + 1);
  const value = (name) => { const at = own.indexOf(name); return at < 0 ? undefined : own[at + 1]; };
  const units = packageCells({ codes: value("--codes")?.split(","), grids: value("--grids")?.split(","), entries: value("--entries")?.split(",") });
  if (own.includes("--list")) {
    for (const unit of units) console.log(`${unit.grid} ${unit.code}: ${unit.ids.length} programs in ${unit.contexts.join(",")}`);
    process.exit(0);
  }
  const out = value("--out");
  const checker = value("--checker-bin"), typefacts = value("--typefacts");
  if (!out || !checker || !typefacts) { console.error("usage: node scripts/fuzz/package-cells.mjs [--list] --out <fresh dir> --checker-bin <solid-checker-rust> --typefacts <solid-typefacts> [--codes A,B] [--grids mx,lg] [--entries a,b] [-- <focus args>]"); process.exit(2); }
  if (existsSync(out)) { console.error(`package-cells: use a fresh --out path (${out} exists)`); process.exit(2); }
  const manifests = join(resolve(out), "manifests");
  mkdirSync(manifests, { recursive: true });
  let status = 0;
  for (const unit of units) {
    const manifest = join(manifests, `${unit.grid}-${unit.code}.json`);
    writeFileSync(manifest, `${JSON.stringify({ generator: GENERATOR_VERSION, seed: "1", ids: unit.ids })}\n`);
    console.log(`=== ${unit.grid} ${unit.code}: ${unit.ids.length} programs`);
    const run = spawnSync(process.execPath, [join(here, "focus.mjs"), "--checker-bin", checker, "--typefacts", typefacts, "--out", join(resolve(out), `${unit.grid}-${unit.code}`), "--codes", unit.code, "--grids", unit.grid, "--runtime-diff", "all", "--seed", "1",
      "--", "--contexts", unit.contexts.join(","), "--ids", manifest, ...passed], { stdio: "inherit" });
    status = Math.max(status, run.status ?? 2);
  }
  process.exit(status);
}
