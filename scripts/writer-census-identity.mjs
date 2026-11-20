#!/usr/bin/env node
// Byte identity of the writer-cause census switch (ADR 1131's M0): the checker's stdout over the corpus, per project, from runs of scripts/writer-census.mjs.
//
//   node scripts/writer-census-identity.mjs --base <run dir> --head <run dir> [--other <run dir>]…
//
// `--base` is the build the branch is cut from, `--head` the branch's build with the census and the what-if off, `--other` any further run that must answer the same
// (the census on). Exits 1 when a project's stdout differs, listing the findings that moved. A project that differs in bytes but not in findings is named apart: the
// checker's JSON carries counters and a metrics object, and one that is not deterministic would differ between two runs of one build.
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";

const args = process.argv.slice(2);
const values = (name) => args.flatMap((arg, at) => (arg === name ? [args[at + 1]] : []));
const base = resolve(values("--base")[0] ?? "");
const runs = [{ name: "head", dir: resolve(values("--head")[0] ?? "") }, ...values("--other").map((dir, index) => ({ name: `other${index + 1}`, dir: resolve(dir) }))];

const meta = (dir, id) => JSON.parse(readFileSync(join(dir, id, "meta.json"), "utf8"));
const ids = readdirSync(base).filter((id) => !id.startsWith(".")).sort();
let different = 0;
let identical = 0;
for (const id of ids) {
  const was = meta(base, id);
  for (const run of runs) {
    const now = meta(run.dir, id);
    if (was.stdoutSha256 === now.stdoutSha256 && was.exitCode === now.exitCode) identical += 1;
    else {
      different += 1;
      console.log(`${run.name} ${id}: base ${was.stdoutSha256.slice(0, 12)} (${was.stdoutBytes} bytes, ${was.findings} findings), ${run.name} ${now.stdoutSha256.slice(0, 12)} (${now.stdoutBytes} bytes, ${now.findings} findings)`);
    }
  }
}
console.log(`${ids.length} projects, ${runs.length} runs against the base: ${identical} identical stdout, ${different} different`);
process.exitCode = different > 0 ? 1 : 0;
