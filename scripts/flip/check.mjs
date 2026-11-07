#!/usr/bin/env node
// node scripts/flip/check.mjs <SCxxxx|all> [--bin <checker>] [--typefacts <producer>] [--tsc <tsrs>] [--jobs N] [--chrome | --recorded]
// Unpacks scripts/flip/witnesses.json into a temp dir, runs the checker on each witness of the rule, and judges it:
//   clean   the site was certified and the runtime complained: must now report the rule, or the `closedBy` code at `closedAt` (line, blocker), never certify.
//   warn    the rule claimed a violation the runtime contradicts: must no longer be a violation of that rule.
//   keep    a true finding: every finding of the rule in `before` must stay, same kind.
//   regress not certified today: must stay not certified.
//   pref    a preference rule's unproved site: must report the rule, uncertifiable.
// Every witness is type-checked first: a program tsc rejects is no defect (invariant 1), so it fails here, whatever the checker says.
// Every kind: where the recorded run fired a code a rule claims, some finding must cover it (judge.mjs, the pattern lane's judge).
// `--recorded` (make verify, which has no tsrs and no Chrome): judges the checker against the recorded runtime and skips the type
//   check; a witness's type result depends only on its bytes and the pinned install. `--chrome` re-runs both.
//   host    hydrate or server host, from scripts/flip/host-witnesses.json (notes/ssr/report.md): `runtime` is what each side measured. A rule's
//           violation needs a code it claims on a path the selection runs; a code a rule claims needs a finding of that rule. `open` names the
//           rule whose commit closes it. `check.mjs HOST` runs them; `--chrome` does not, they have their own hosts.
// `--chrome` also runs each in real Chrome (run.mjs) and fails where the runtime codes differ from `runtime.diagnostics`.
// --jobs N checker and Chrome workers, default half the cores.
// FLIP_INSTALL overrides the oracle install, FLIP_TMP the temp root. Exit 1 on any failure.
import { mkdirSync, writeFileSync, readFileSync, realpathSync, rmSync, symlinkSync, existsSync, mkdtempSync } from "node:fs";
import { spawn, spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { availableParallelism, tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { resolveTsc, requireTypeChecker, tsconfigFor, typecheck, typecheckBatch } from "../fuzz/typecheck.mjs";
import { judgeWitness, vanishedRows } from "./judge.mjs";
const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "../..");
const argv = process.argv.slice(2);
const opt = (k, d) => { const i = argv.indexOf(k); return i < 0 ? d : argv[i + 1]; };
const rule = argv[0] ?? "all";
const bin = opt("--bin", join(repo, "rust/target/release/solid-checker-rust"));
const typefacts = opt("--typefacts", join(repo, "bin/solid-typefacts"));
const recorded = argv.includes("--recorded");
if (recorded && argv.includes("--chrome")) throw new Error("--recorded and --chrome exclude each other");
const tsc = recorded ? null : resolveTsc(opt("--tsc"));
if (!recorded) requireTypeChecker(tsc);
if (!existsSync(bin) || !existsSync(typefacts)) throw new Error(`no checker at ${bin} or no producer at ${typefacts}: make build-checker-release`);
const install = process.env.FLIP_INSTALL ?? join(repo, "scripts/runtime-oracle-install");
const W = { ...JSON.parse(readFileSync(join(here, "witnesses.json"), "utf8")), ...JSON.parse(readFileSync(join(here, "host-witnesses.json"), "utf8")) };
const jobs = Number(opt("--jobs", Math.max(1, Math.floor(availableParallelism() / 2))));
// Real path: tsrs prints resolved paths, and the batch answer matches them against these directories.
const work = realpathSync(mkdtempSync(join(process.env.FLIP_TMP ?? tmpdir(), "flip-")));
const ids = Object.keys(W).filter((id) => rule === "all" || W[id].rule === rule);
let fail = 0; const rows = [];
const dirOf = (id) => join(work, id.replace("/", "_"));
for (const id of ids) {
  const dir = dirOf(id); mkdirSync(dir, { recursive: true });
  const files = W[id].files ?? { "App.tsx": W[id].source, "main.tsx": 'import { render } from "@solidjs/web";\nimport { App } from "./App";\nrender(() => <App />, document.body);\n' };
  for (const [name, text] of Object.entries(files)) writeFileSync(join(dir, name), text);
  writeFileSync(join(dir, "tsconfig.json"), tsconfigFor(Object.keys(files)));
}
if (!existsSync(join(work, "node_modules"))) symlinkSync(join(install, "node_modules"), join(work, "node_modules"));
const types = new Map();
if (recorded) for (const id of ids) types.set(id, { ok: true });
else {
  const checked = await typecheckBatch(ids.map(dirOf), { tsc, timeoutMs: 600000 });
  for (const id of ids) types.set(id, checked?.get(resolve(dirOf(id))) ?? await typecheck(dirOf(id), { tsc }));
}
const runChecker = (dir, mode) => new Promise((done) => {
  const child = spawn(bin, ["--format", "json", "--internal", "--project", "tsconfig.json", ...mode], { cwd: dir, stdio: ["ignore", "pipe", "pipe"], env: { ...process.env, SOLID_CHECKER_DAEMON: "0", SOLID_TYPEFACTS_BIN: typefacts } });
  let stdout = "", stderr = "";
  child.stdout.on("data", (chunk) => { stdout += chunk; });
  child.stderr.on("data", (chunk) => { stderr += chunk; });
  child.on("close", () => done({ stdout, stderr }));
});
async function judge(id) {
  const w = W[id]; const dir = dirOf(id);
  const typed = types.get(id);
  if (!typed.ok) { fail++; rows.push(`TYPE ${id}: ${(typed.errors[0] ?? "no answer").slice(0, 140)}`); return; }
  const mode = w.host === "hydrated" ? ["--runtime-build", "development", "--rendering", "hydrated"]
    : w.mode === "ssr" || w.host === "server" ? ["--runtime-target", "node", "--runtime-build", "development", "--rendering", "streaming-ssr"]
      : ["--runtime-target", "browser", "--runtime-build", "development", "--rendering", "csr"];
  const r = await runChecker(dir, mode);
  let out; try { out = JSON.parse(r.stdout); } catch { rows.push(`ERROR ${id}: ${(r.stderr ?? "").slice(0, 200)}`); fail++; return; }
  const verdict = judgeWitness(w, out);
  if (!verdict.ok) { fail++; rows.push(`FAIL ${(w.host ? "host" : w.kind).padEnd(7)} ${id}: ${verdict.why}`); }
}
let next = 0;
await Promise.all(Array.from({ length: Math.min(jobs, ids.length) }, async () => { while (next < ids.length) await judge(ids[next++]); }));
if (argv.includes("--chrome")) {
  const cases = join(work, "cases"); mkdirSync(cases, { recursive: true });
  const browser = ids.filter((id) => !W[id].host && W[id].mode !== "ssr" && W[id].runtime.diagnostics && types.get(id).ok);
  for (const id of browser) { mkdirSync(join(cases, id.replace("/", "_")), { recursive: true }); writeFileSync(join(cases, id.replace("/", "_"), "App.tsx"), W[id].source); }
  const results = join(work, "chrome.json");
  const run = spawnSync(process.execPath, [join(here, "run.mjs"), cases, results, "--jobs", String(jobs)], { encoding: "utf8", env: process.env, stdio: ["ignore", "pipe", "inherit"], maxBuffer: 1 << 26 });
  if (!existsSync(results)) { fail++; rows.push(`CHROME run.mjs wrote nothing: ${(run.stdout ?? "").slice(-300)}`); } else {
    const seen = JSON.parse(readFileSync(results, "utf8"));
    const same = (a, b) => [...new Set(a)].sort().join() === [...new Set(b)].sort().join();
    for (const id of browser) {
      const got = seen[id.replace("/", "_")];
      if (!got || got.failure) { fail++; rows.push(`CHROME ${id}: ${got?.failure?.slice(0, 120) ?? "no record"}`); continue; }
      const codes = (got.diagnostics ?? []).map((d) => d.code);
      if (!same(codes, W[id].runtime.diagnostics)) { fail++; rows.push(`CHROME ${id}: ${codes.join(" ") || "quiet"}, recorded ${W[id].runtime.diagnostics.join(" ") || "quiet"}`); }
    }
  }
}
rmSync(work, { recursive: true, force: true });
// Rows may not vanish against the merge base with main (`WITNESS_BASE` overrides), or a retired row names an ADR (judge.mjs).
const retired = JSON.parse(readFileSync(join(here, "witnesses-retired.json"), "utf8"));
if (rule === "all") {
  const git = (...args) => spawnSync("git", args, { cwd: repo, encoding: "utf8", maxBuffer: 1 << 28 });
  // The merge base with main, or, while main has no witnesses.json, the commit that added it (review item 8: the guard compared nothing).
  const first = () => git("log", "--diff-filter=A", "--format=%H", "--", "scripts/flip/witnesses.json").stdout.trim().split("\n").pop();
  const main = git("merge-base", "HEAD", "origin/main").stdout.trim();
  const base = process.env.WITNESS_BASE ?? (main && git("cat-file", "-e", `${main}:scripts/flip/witnesses.json`).status === 0 ? main : first());
  const at = (file) => { const r = git("show", `${base}:scripts/flip/${file}`); return r.status === 0 ? JSON.parse(r.stdout) : null; };
  const was = base ? at("witnesses.json") : null;
  if (!was) console.log(`witnesses: no witnesses.json at base ${base || "(none)"}, no row compared`);
  else {
    const before = { ...was, ...(at("host-witnesses.json") ?? {}) };
    for (const why of vanishedRows(before, W, retired, readFileSync(join(repo, "docs/decisions.md"), "utf8"))) { fail++; rows.push(`VANISHED ${why}`); }
  }
}
console.log(`witnesses: ${retired.length} retired`);
for (const line of rows.sort()) console.log(line);
console.log(`${ids.length - fail} of ${ids.length} hold${rule === "all" ? "" : ` for ${rule}`}`);
process.exitCode = fail ? 1 : 0;
