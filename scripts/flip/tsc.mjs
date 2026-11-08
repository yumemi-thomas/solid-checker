#!/usr/bin/env node
// node scripts/flip/tsc.mjs [--tsc <tsrs>] <root>...   type-checks every <root>/**/App.tsx program with tsrs, the project's tsc, in a copy.
// Prints each program that fails and its first error. Exit 1 if any fails, 2 if tsrs is missing or a program got no answer. Replaces the
// audits' tsc.sh, which printed `tsc=0` for every program: it read `$?` after `$(basename)` had reset it.
import { cpSync, mkdirSync, mkdtempSync, readdirSync, realpathSync, rmSync, statSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { requireTypeChecker, resolveTsc, tsconfigFor, typecheck, typecheckBatch } from "../fuzz/typecheck.mjs";

const argv = process.argv.slice(2);
const at = argv.indexOf("--tsc");
const tsc = resolveTsc(at < 0 ? undefined : argv[at + 1]);
const roots = argv.filter((_, index) => at < 0 || (index !== at && index !== at + 1));
requireTypeChecker(tsc);
const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const install = process.env.FLIP_INSTALL ?? join(repo, "scripts/runtime-oracle-install");
const programs = [];
const walk = (dir) => { for (const name of readdirSync(dir)) { if (name === "node_modules") continue; const path = join(dir, name); if (statSync(path).isDirectory()) walk(path); else if (name === "App.tsx") programs.push(dir); } };
for (const root of roots) walk(resolve(root));
const work = realpathSync(mkdtempSync(join(process.env.FLIP_TMP ?? tmpdir(), "flip-tsc-")));
symlinkSync(join(install, "node_modules"), join(work, "node_modules"));
const copies = programs.map((dir, index) => { const copy = join(work, `p${index}`); mkdirSync(copy); cpSync(dir, copy, { recursive: true, verbatimSymlinks: true }); writeFileSync(join(copy, "main.tsx"), 'import { render } from "@solidjs/web";\nimport { App } from "./App";\nrender(() => <App />, document.body);\n'); writeFileSync(join(copy, "tsconfig.json"), tsconfigFor(readdirSync(copy).filter((f) => /\.tsx?$/.test(f)))); return copy; });
let failed = 0, unanswered = 0;
for (let start = 0; start < copies.length; start += 100) {
  const batch = copies.slice(start, start + 100);
  const answers = await typecheckBatch(batch, { tsc, timeoutMs: 600000 });
  for (const copy of batch) {
    const answer = answers?.get(resolve(copy)) ?? await typecheck(copy, { tsc });
    if (answer.missing) { unanswered += 1; continue; }
    if (!answer.ok) { failed += 1; console.log(`${relative(process.cwd(), programs[copies.indexOf(copy)])}: ${(answer.errors[0] ?? "failed with no error line").slice(0, 150)}`); }
  }
}
rmSync(work, { recursive: true, force: true });
console.log(`${programs.length - failed - unanswered} of ${programs.length} pass`);
process.exitCode = unanswered ? 2 : failed ? 1 : 0;
