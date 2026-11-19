#!/usr/bin/env node
// The textbook tier of the proved ledger (ADR 1118): `fixtures/textbook/`, idiomatic Solid 2 programs from the official templates, the cheatsheet and our
// own idioms, every one legal (tsc-clean against rc.21, quiet in Chrome when admitted). The ledger is `scripts/fuzz/proved/textbook.json`:
// per program its content hash, whether the checker certified it, and the (site, code) keys it left uncertifiable (`u`). Every other (site, code) of
// a legal program is proved clean, so:
//   - a violation anywhere is a false positive (the program is legal): a failure, never nameable;
//   - a key that is not in `u`, or a certified program that is not certified now, is a lost proof: a failure unless a fix row of
//     `scripts/fuzz/proved/changes.json` names it (`grid: "textbook"`, `keys: ["<program>|<key>"]` or `ids: ["<program>"]`);
//   - a key of `u` that left is a gain; `record` writes it, and never adds a key to `u` (a loss is named, not recorded).
// A program whose content hash differs from its entry is new bytes (an edit of the set): its entry is written by `record --admit <program>` only.
//
//   node scripts/fuzz/textbook.mjs check|record --checker <bin> --typefacts <bin> [--admit <program>,...]
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { check as runCheck, programsOf, siteCodes } from "../monotone.mjs";
import { PROVED_DIR, namedIds, readChanges } from "./proved.mjs";

const ROOT = resolve(fileURLToPath(new URL(".", import.meta.url)), "../..");
const SET = join(ROOT, "fixtures", "textbook");
export const TEXTBOOK = join(PROVED_DIR, "textbook.json");

/** A textbook program's content hash: its files, sorted by name. */
export function textbookHash(name, dir = SET) {
  const files = readdirSync(join(dir, name)).sort().map((file) => [file, readFileSync(join(dir, name, file), "utf8")]);
  return createHash("sha256").update(JSON.stringify(files)).digest("hex").slice(0, 20);
}

/**
 * One program's result against its entry: `{ falsePositives, lost, gained }`. `now`: `{ certified, sites: Map(key -> V|U) }`; `entry`: `{ certified, u }`.
 */
export function judgeTextbook(now, entry) {
  const falsePositives = [...now.sites].filter(([, kind]) => kind === "V").map(([key]) => key);
  const u = new Set(entry.u);
  const lost = [...now.sites].filter(([key, kind]) => kind === "U" && !u.has(key)).map(([key]) => key);
  if (entry.certified && !now.certified && !lost.length) lost.push("(certified)");
  const gained = [...u].filter((key) => !now.sites.has(key));
  return { falsePositives, lost, gained };
}

/** Read the ledger, `{ programs: { name: { hash, certified, u } } }`. */
export const readTextbook = (path = TEXTBOOK) => (existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : { programs: {} });

/** The ledger's text: programs and keys sorted, one program a line. */
export const textbookText = (ledger) => `{"programs":{\n${Object.keys(ledger.programs).sort().map((name) => { const e = ledger.programs[name]; return `${JSON.stringify(name)}:${JSON.stringify({ hash: e.hash, certified: e.certified, u: [...e.u].sort() })}`; }).join(",\n")}\n}}\n`;

async function main(argv) {
  const value = (name) => { const at = argv.indexOf(name); return at < 0 ? undefined : argv[at + 1]; };
  const command = argv[0], checker = value("--checker"), typefacts = value("--typefacts");
  if (!["check", "record"].includes(command) || !checker || !typefacts) throw new Error("usage: textbook.mjs check|record --checker <bin> --typefacts <bin> [--admit a,b]");
  const admit = new Set((value("--admit") ?? "").split(",").filter(Boolean));
  const ledger = readTextbook();
  const names = new Set((namedIds(readChanges(), "textbook")));
  const programs = await programsOf("textbook", { install: join(ROOT, "scripts", "fuzz", "install") });
  const problems = [];
  let certified = 0, uKeys = 0, gainedTotal = 0;
  for (const program of programs) {
    const name = program.id.replace(/^textbook\//, "");
    const project = program.prepare();
    let report;
    try { report = await runCheck(checker, { ...project, typefacts }); } finally { project.cleanup(); }
    // Certified: no (site, code) of the program's own files (the harness entry's rows are the entry's, `siteCodes` leaves them out).
    const sites = siteCodes(report, project.root);
    const now = { certified: sites.size === 0, sites };
    certified += now.certified ? 1 : 0;
    uKeys += [...now.sites.values()].filter((kind) => kind === "U").length;
    const hash = textbookHash(name);
    const entry = ledger.programs[name];
    if (!entry || entry.hash !== hash) {
      if (command === "record" && admit.has(name)) { ledger.programs[name] = { hash, certified: now.certified, u: [...now.sites.keys()] }; continue; }
      problems.push(`textbook ${name}: ${entry ? "its bytes changed" : "not in the ledger"}: admit it with \`record --admit ${name}\` (after tsc and Chrome)`);
      continue;
    }
    const { falsePositives, lost, gained } = judgeTextbook(now, entry);
    for (const key of falsePositives) problems.push(`textbook ${name}: a violation on a legal program (false positive): ${key}`);
    const unnamed = lost.filter((key) => !names.has(name) && !names.has(`${name}|${key}`));
    for (const key of unnamed) problems.push(`textbook ${name}: proved (site, code) lost, now uncertifiable: ${key} (a fix row of proved/changes.json, grid "textbook", names it)`);
    gainedTotal += gained.length;
    if (command === "record" && !falsePositives.length && !unnamed.length) ledger.programs[name] = { hash, certified: entry.certified || now.certified, u: entry.u.filter((key) => now.sites.has(key)) };
  }
  if (command === "record") writeFileSync(TEXTBOOK, textbookText(ledger));
  console.log(`textbook: ${programs.length} programs, ${certified} certified, ${uKeys} uncertifiable (site, code), ${gainedTotal} newly proved; ${problems.length} problem(s)`);
  for (const line of problems) console.log(`  ${line}`);
  process.exitCode = problems.length ? 1 : 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main(process.argv.slice(2)).catch((error) => { console.error(error.message); process.exitCode = 2; });
