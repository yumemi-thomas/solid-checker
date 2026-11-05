#!/usr/bin/env node
// node scripts/flip/check.mjs <SCxxxx|all> [--bin <checker>] [--typefacts <producer>]
// Unpacks scripts/flip/witnesses.json into a temp dir, runs the checker on each witness of the rule, and judges it:
//   clean   the site was certified and the runtime complained: must now report the rule (or SC9012 for census rows), never certify.
//   warn    the rule claimed a violation the runtime contradicts: must no longer be a violation of that rule.
//   keep    a true finding: every finding of the rule in `before` must stay, same kind.
//   regress not certified today: must stay not certified.
//   pref    a preference rule's unproved site: must report the rule, uncertifiable.
import { mkdirSync, writeFileSync, readFileSync, symlinkSync, existsSync, mkdtempSync } from "node:fs";
import { execFileSync, spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "../..");
const argv = process.argv.slice(2);
const opt = (k, d) => { const i = argv.indexOf(k); return i < 0 ? d : argv[i + 1]; };
const rule = argv[0] ?? "all";
const bin = opt("--bin", join(repo, "rust/target/release/solid-checker-rust"));
const typefacts = opt("--typefacts", join(repo, "bin/solid-typefacts"));
const install = process.env.FLIP_INSTALL ?? join(repo, "scripts/runtime-oracle-install");
const W = JSON.parse(readFileSync(join(here, "witnesses.json"), "utf8"));
const work = mkdtempSync(join(process.env.FLIP_TMP ?? tmpdir(), "flip-"));
const ids = Object.keys(W).filter((id) => rule === "all" || W[id].rule === rule);
let fail = 0; const rows = [];
for (const id of ids) {
  const w = W[id]; const dir = join(work, id.replace("/", "_")); mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "App.tsx"), w.source);
  writeFileSync(join(dir, "main.tsx"), 'import { render } from "@solidjs/web";\nimport { App } from "./App";\nrender(() => <App />, document.body);\n');
  writeFileSync(join(dir, "tsconfig.json"), JSON.stringify({ compilerOptions: { jsx: "preserve", jsxImportSource: "@solidjs/web", module: "ESNext", moduleResolution: "Bundler", strict: true, target: "ES2022", lib: ["ES2022", "DOM"], types: [] }, include: ["main.tsx", "App.tsx"] }));
  if (!existsSync(join(dir, "node_modules"))) symlinkSync(join(install, "node_modules"), join(dir, "node_modules"));
  const mode = w.mode === "ssr" ? ["--runtime-target", "node", "--runtime-build", "development", "--rendering", "streaming-ssr"] : ["--runtime-target", "browser", "--runtime-build", "development", "--rendering", "csr"];
  const r = spawnSync(bin, ["--format", "json", "--internal", "--project", "tsconfig.json", ...mode], { cwd: dir, encoding: "utf8", env: { ...process.env, SOLID_CHECKER_DAEMON: "0", SOLID_TYPEFACTS_BIN: typefacts }, maxBuffer: 1 << 26 });
  let out; try { out = JSON.parse(r.stdout); } catch { rows.push(`ERROR ${id}: ${(r.stderr ?? "").slice(0, 200)}`); fail++; continue; }
  const own = (out.findings ?? []).filter((f) => f.id === w.rule || (w.rule === "SC2004" && f.id === "SC2005"));
  const now = (out.findings ?? []).map((f) => `${f.id}/${f.kind[0]}@${f.primaryLocation?.line}`);
  let ok;
  if (w.kind === "clean") ok = w.rule === "SC9012" ? out.status !== "certified" : own.length > 0;
  else if (w.kind === "pref") ok = own.some((f) => f.kind === "uncertifiable");
  else if (w.kind === "warn") ok = !own.some((f) => f.kind === "violation");
  else if (w.kind === "keep") ok = (w.before.findings ?? []).filter((f) => f.startsWith(w.rule)).every((f) => now.includes(f));
  else ok = out.status !== "certified";
  if (!ok) { fail++; rows.push(`FAIL ${w.kind.padEnd(7)} ${id}: ${out.status} ${now.join(" ") || "no findings"}`); }
}
for (const line of rows) console.log(line);
console.log(`${ids.length - fail} of ${ids.length} hold${rule === "all" ? "" : ` for ${rule}`}`);
process.exitCode = fail ? 1 : 0;
