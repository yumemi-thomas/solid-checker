// Scratch, not in the repo: run one checker build over fuzz pattern dirs and print every finding's rule, kind, file:line and blocker,
// then whether a row stands on each pattern's read line (hand-read, SITES below).
//   node sites.mjs <checker-bin> <typefacts-bin> <patterns-root> <out.json> [ids...]
import { spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readdirSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [checker, typefacts, root, out, ...only] = process.argv.slice(2);
const INSTALL = "/Users/thomas.guehenneux/EDF/Yumemi/GitHub/solid-checker/scripts/runtime-oracle-install";
const TSCONFIG = JSON.stringify({ compilerOptions: { jsx: "preserve", jsxImportSource: "@solidjs/web", module: "ESNext", moduleResolution: "Bundler", strict: true, target: "ES2022", lib: ["ES2022", "DOM", "DOM.Iterable"], skipLibCheck: true, noEmit: true }, include: ["*.ts", "*.tsx"] });
// The read (or write) Chrome reports, read by hand: file and line.
export const SITES = {
  "api/v01-store-map-get": ["App.tsx", 11], "api/v02-store-nullish": ["App.tsx", 11], "api/v08-store-write-map": ["App.tsx", 11],
  "api/v20-props-map-get": ["App.tsx", 11], "api/v21-props-nullish": ["App.tsx", 11], "census/a05-object-assign": ["App.tsx", 12],
  "census/a19-globalthis": ["App.tsx", 12], "census/k13-untrack-returns-accessor": ["App.tsx", 10], "census/k14-runWithOwner-returns-accessor": ["App.tsx", 10],
  "returns/fc-05-array-foreach": ["App.tsx", 8], "returns/fc-15-sibling-this": ["App.tsx", 11], "returns/r05-array-real-assign": ["App.tsx", 7],
  "returns/r05c-array-real-assign-index": ["App.tsx", 7], "returns/r05d-array-slot-closure-assign": ["App.tsx", 7],
  "values/d8-pkg-global-named-fn": ["App.tsx", 4], "values/d8-pkg-side-effect-import": ["App.tsx", 4], "values/d8-pkg-window-read": ["App.tsx", 4],
  "values/r2-hook-store-module": ["App.tsx", 3],
};
const ids = only.length ? only : Object.keys(SITES);
const work = `${out}.work`;
rmSync(work, { recursive: true, force: true });
const results = {};
for (const id of ids) {
  const dir = join(work, id.replace("/", "--"));
  cpSync(join(root, id), dir, { recursive: true });
  mkdirSync(join(dir, "node_modules"), { recursive: true });
  for (const name of readdirSync(join(INSTALL, "node_modules"))) if (!existsSync(join(dir, "node_modules", name))) symlinkSync(join(INSTALL, "node_modules", name), join(dir, "node_modules", name));
  writeFileSync(join(dir, "main.tsx"), 'import { render } from "@solidjs/web";\nimport { App } from "./App";\nrender(() => <App />, document.body);\n');
  writeFileSync(join(dir, "tsconfig.json"), TSCONFIG);
  const run = spawnSync(checker, ["--project", join(dir, "tsconfig.json"), "--format", "json", "--runtime-target", "browser", "--runtime-build", "development", "--rendering", "csr"], { env: { ...process.env, SOLID_TYPEFACTS_BIN: typefacts, SOLID_CHECKER_DAEMON: "0" }, encoding: "utf8", maxBuffer: 1 << 28 });
  let findings;
  try { const report = JSON.parse(run.stdout); findings = (report.analysis?.findings ?? report.findings ?? []); } catch { findings = null; }
  const rows = (findings ?? []).map((f) => ({ rule: f.rule, kind: f.kind, at: `${(f.primaryLocation?.path ?? "").split("/").pop()}:${f.primaryLocation?.line}`, blocker: String(f.analysisContext ?? "").split(":")[0] }))
    .filter((r) => !r.at.startsWith("main.tsx"));
  const [file, line] = SITES[id] ?? [];
  const atSite = rows.filter((r) => r.at === `${file}:${line}` && (r.kind === "uncertifiable" || r.kind === "violation"));
  results[id] = { site: `${file}:${line}`, atSite: atSite.length, rows, ...(findings === null ? { failed: run.stderr.slice(0, 400) } : {}) };
  console.log(`${atSite.length ? "ROW " : "NONE"} ${id} site ${file}:${line}  ${rows.map((r) => `${r.rule}/${r.kind}@${r.at}[${r.blocker}]`).join(" ")}`);
}
writeFileSync(out, JSON.stringify(results, null, 1));
