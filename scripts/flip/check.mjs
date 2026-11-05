#!/usr/bin/env node
// node scripts/flip/check.mjs <SCxxxx|all> [--bin <checker>] [--typefacts <producer>] [--tsc <tsrs>] [--jobs N] [--chrome]
// Unpacks scripts/flip/witnesses.json into a temp dir, runs the checker on each witness of the rule, and judges it:
//   clean   the site was certified and the runtime complained: must now report the rule (or SC9012 for census rows), never certify.
//   warn    the rule claimed a violation the runtime contradicts: must no longer be a violation of that rule.
//   keep    a true finding: every finding of the rule in `before` must stay, same kind.
//   regress not certified today: must stay not certified.
//   pref    a preference rule's unproved site: must report the rule, uncertifiable.
// Every witness is type-checked first: a program tsc rejects is no defect (invariant 1), so it fails here, whatever the checker says.
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
import { codeRules, ruleHasRuntimeCounterpart } from "../fuzz/codes.mjs";
const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "../..");
const argv = process.argv.slice(2);
const opt = (k, d) => { const i = argv.indexOf(k); return i < 0 ? d : argv[i + 1]; };
const rule = argv[0] ?? "all";
const bin = opt("--bin", join(repo, "rust/target/release/solid-checker-rust"));
const typefacts = opt("--typefacts", join(repo, "bin/solid-typefacts"));
const tsc = resolveTsc(opt("--tsc"));
requireTypeChecker(tsc);
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
const checked = await typecheckBatch(ids.map(dirOf), { tsc, timeoutMs: 600000 });
const types = new Map();
for (const id of ids) types.set(id, checked?.get(resolve(dirOf(id))) ?? await typecheck(dirOf(id), { tsc }));
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
  const own = (out.findings ?? []).filter((f) => f.id === w.rule || (w.rule === "SC2004" && f.id === "SC2005"));
  const now = (out.findings ?? []).map((f) => `${f.id}/${f.kind[0]}@${f.primaryLocation?.line}`);
  const was = w.before?.findings ?? [];
  let ok;
  if (w.host) {
    const seen = [...(w.runtime.client ?? []), ...(w.runtime.server ?? [])];
    const claimed = new Set(seen.flatMap((code) => codeRules[code] ?? []));
    const unearned = (out.findings ?? []).filter((f) => f.kind === "violation" && ruleHasRuntimeCounterpart(f.rule) && !claimed.has(f.rule));
    const missed = [...claimed].filter((rule) => !(out.findings ?? []).some((f) => f.rule === rule));
    ok = !unearned.length && !missed.length;
    if (!ok) { fail++; rows.push(`FAIL host    ${id}: ${w.open ? `open (${w.open}) ` : ""}unearned ${unearned.map((f) => f.id).join(" ") || "none"}, missed ${missed.join(" ") || "none"}; ${now.join(" ") || "no findings"}`); }
    return;
  }
  if (w.kind === "clean") ok = w.rule === "SC9012" ? out.status !== "certified" : own.length > 0;
  else if (w.kind === "pref") ok = own.some((f) => f.kind === "uncertifiable");
  else if (w.kind === "warn") ok = !own.some((f) => f.kind === "violation");
  else if (w.kind === "keep") ok = was.filter((f) => f.startsWith(w.rule)).every((f) => now.includes(f));
  else ok = out.status !== "certified";
  if (!ok) { fail++; rows.push(`FAIL ${w.kind.padEnd(7)} ${id}: ${out.status} ${now.join(" ") || "no findings"}`); }
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
for (const line of rows.sort()) console.log(line);
console.log(`${ids.length - fail} of ${ids.length} hold${rule === "all" ? "" : ` for ${rule}`}`);
process.exitCode = fail ? 1 : 0;
