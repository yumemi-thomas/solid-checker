#!/usr/bin/env node
// ADR 1114 families 2-4, step 2: the references of every reactive source by position, measured (no verdict changes). The checker writes two
// measurement lines per reference when SOLID_CHECKER_IR_PROOF_CANDIDATES and the proof log are on: the site universe's (`rule: universe`, its
// position by its parent node, facts schema 102, and how many universe sites start at it) and the value-use census's (`rule: census`, its
// answer: `followed` or the open obligation's blocker, and how many uses its walk ended at). This joins them by file and span and counts, per
// (source, position):
//   site        the universe has a site at the reference;
//   open        the census answered an obligation;
//   uses        the census followed the value and filed obligations at the uses it ended at;
//   silent      no universe site, and the census followed the value with no obligation anywhere: the reference's silence rests on the
//               census's follow rules alone (the positions ADR 1114 families 2-4 make sites of);
//   no-census   a universe line with no census line at its span: neither the universe nor the census answers the reference.
//
//   node scripts/fuzz/reference-census.mjs --checker <bin> --typefacts <bin> --set textbook [--set fixtures] [--set grid:lg-base:500] --out <report.json>
//   node scripts/fuzz/reference-census.mjs --logs <dir> --out <report.json>     (fold logs a run already wrote)
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { mapPoolSettled } from "../lib/pool.mjs";

/** The joined references of one log's text: `[{ path, s, e, position, source, sites, answer, uses, escaped }]`. */
export function joinLog(text) {
  const paths = new Map();
  const universe = new Map();
  const census = new Map();
  for (const line of text.split("\n")) {
    if (!line) continue;
    const entry = JSON.parse(line);
    if (entry.k === "path") { paths.set(`${entry.q}:${entry.p}`, entry.f); continue; }
    if (entry.k !== "candidate" || entry.t !== "reference") continue;
    const key = `${paths.get(`${entry.q}:${entry.p}`) ?? `${entry.q}:${entry.p}`}\u0000${entry.s}\u0000${entry.e}`;
    if (entry.rule === "universe") universe.set(key, entry.facts);
    else if (entry.rule === "census") census.set(key, entry.facts);
  }
  const out = [];
  for (const [key, facts] of universe) {
    const [path, s, e] = key.split("\u0000");
    const answer = census.get(key);
    out.push({ path, s: Number(s), e: Number(e), position: facts.position, source: facts.source, sites: Number(facts.sites), answer: answer?.answer ?? null, uses: Number(answer?.uses ?? 0), escaped: Number(answer?.escaped ?? 0) });
  }
  return out;
}

/** The class of a joined reference (see the header). */
export function classOf(reference) {
  if (reference.sites > 0) return "site";
  if (reference.answer === null) return "no-census";
  if (reference.answer !== "followed") return "open";
  return reference.uses > 0 || reference.escaped > 0 ? "uses" : "silent";
}

/** The tally: `{ rows: [{ source, position, total, site, open, followed, unanswered, no-census, examples }] }`, the most unanswered first. */
export function tally(references, examples = 3) {
  const rows = new Map();
  for (const reference of references) {
    // `Argument { index: 2 }` is one position whatever the index; `Initializer { pattern: true }` keeps its flag.
    const position = reference.position.replace(/ \{ index: \d+ \}$/, "");
    const key = `${reference.source}\u0000${position}`;
    const row = rows.get(key) ?? { source: reference.source, position, total: 0, site: 0, open: 0, uses: 0, silent: 0, "no-census": 0, examples: [] };
    const kind = classOf(reference);
    row.total += 1;
    row[kind] += 1;
    if ((kind === "silent" || kind === "no-census") && row.examples.length < examples) row.examples.push(`${reference.path.replace(/^.*\/(fixtures|cases|\.solid-checker-fuzz)\//, "")}:${reference.s}-${reference.e}`);
    rows.set(key, row);
  }
  return [...rows.values()].sort((a, b) => b["no-census"] - a["no-census"] || b.silent - a.silent || b.total - a.total || (a.source + a.position < b.source + b.position ? -1 : 1));
}

async function main(argv) {
  const values = (name) => argv.flatMap((arg, at) => (arg === name ? [argv[at + 1]] : []));
  const value = (name) => values(name)[0];
  const out = value("--out");
  if (!out) throw new Error("usage: reference-census.mjs (--checker <bin> --typefacts <bin> --set <set>... | --logs <dir>) --out <report.json>");
  const references = [];
  let programs = 0;
  if (value("--logs")) {
    for (const name of readdirSync(value("--logs"))) references.push(...joinLog(readFileSync(join(value("--logs"), name), "utf8")));
  } else {
    const { check, programsOf } = await import("../monotone.mjs");
    const install = resolve(value("--install") ?? join(fileURLToPath(new URL(".", import.meta.url)), "install"));
    const list = [];
    for (const set of values("--set")) list.push(...(await programsOf(set, { install })));
    programs = list.length;
    process.env.SOLID_CHECKER_IR_PROOF_CANDIDATES = "1";
    const results = await mapPoolSettled(list, async (program) => {
      const project = program.prepare();
      const log = mkdtempSync(join(tmpdir(), "reference-census-"));
      try {
        await check(value("--checker"), { ...project, typefacts: value("--typefacts"), proofLog: log });
        return readdirSync(log).flatMap((name) => joinLog(readFileSync(join(log, name), "utf8")).map((reference) => ({ ...reference, program: program.id })));
      } finally {
        rmSync(log, { recursive: true, force: true });
        project.cleanup();
      }
    }, { concurrency: Number(value("--concurrency") ?? 4) });
    for (const result of results) if (result.value) references.push(...result.value.filter((reference) => !/\/main\.tsx$/.test(reference.path) || !reference.path.includes(".solid-checker-fuzz")));
  }
  const rows = tally(references);
  const totals = rows.reduce((sum, row) => { for (const k of ["total", "site", "open", "uses", "silent", "no-census"]) sum[k] = (sum[k] ?? 0) + row[k]; return sum; }, {});
  if (existsSync(out)) throw new Error(`use a fresh --out path (${out} exists)`);
  writeFileSync(out, `${JSON.stringify({ sets: values("--set"), programs, totals, rows }, null, 2)}\n`);
  console.log(`reference census: ${programs} programs, ${totals.total ?? 0} references: site ${totals.site ?? 0}, census open ${totals.open ?? 0}, census uses ${totals.uses ?? 0}, census silent ${totals.silent ?? 0}, answered by neither ${totals["no-census"] ?? 0}`);
  console.log("  source    position                             total   site   open   uses SILENT NEITHER");
  for (const row of rows.slice(0, 45)) console.log(`  ${row.source.padEnd(9)} ${row.position.padEnd(35)} ${String(row.total).padStart(6)} ${String(row.site).padStart(6)} ${String(row.open).padStart(6)} ${String(row.uses).padStart(6)} ${String(row.silent).padStart(6)} ${String(row["no-census"]).padStart(7)}${row.examples.length ? `  e.g. ${row.examples[0]}` : ""}`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main(process.argv.slice(2)).catch((error) => { console.error(error.message); process.exitCode = 2; });
