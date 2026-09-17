import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { cpSync, rmSync, mkdirSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { join, dirname } from "node:path";
const run = promisify(execFile);
const root = process.env.REPO;
const scratch = process.env.SCRATCH;
const checker = join(root, "rust/target/debug/solid-checker-rust");
const typefacts = join(root, "bin/solid-typefacts");

const projects = process.env.PROJECTS.trim().split("\n").filter(Boolean);

async function analyze(tsconfig) {
  try {
    const { stdout } = await run(checker, ["--format", "json", "--project", tsconfig], {
      cwd: root, maxBuffer: 256 * 1024 * 1024,
      env: { ...process.env, SOLID_TYPEFACTS_BIN: typefacts }
    });
    const s = JSON.parse(stdout);
    return { status: s.status, findings: (s.findings ?? []).map(f => ({
      id: f.id, rule: f.rule, kind: f.kind,
      loc: `${(f.primaryLocation?.path ?? "").split("/").pop()}:${f.primaryLocation?.line}:${f.primaryLocation?.column}`,
      ctx: f.analysisContext ?? ""
    })) };
  } catch (e) { return { status: "ERROR", error: String(e.message).slice(0, 200), findings: [] }; }
}

const key = f => `${f.id}|${f.rule}|${f.kind}|${f.loc}`;
const out = [];
const base = join(scratch, "runs");
rmSync(base, { recursive: true, force: true });
mkdirSync(base, { recursive: true });

for (const catalogPath of projects) {
  const projDir = dirname(dirname(catalogPath));           // <fixture>/.solid-checker/x.json -> <fixture>
  const id = projDir.replace(/^fixtures\//, "").replace(/\//g, "__");
  const tsconfig = join(root, projDir, "tsconfig.json");
  if (!existsSync(tsconfig)) { out.push({ id, skipped: "no tsconfig" }); continue; }
  const on = join(base, id, "on"), off = join(base, id, "off");
  cpSync(join(root, projDir), on, { recursive: true });
  cpSync(join(root, projDir), off, { recursive: true });
  rmSync(join(off, ".solid-checker/accepted-contracts.json"), { force: true });

  const [a, b] = [await analyze(join(on, "tsconfig.json")), await analyze(join(off, "tsconfig.json"))];
  const onKeys = new Map(a.findings.map(f => [key(f), f]));
  const offKeys = new Map(b.findings.map(f => [key(f), f]));
  const added = [...onKeys.keys()].filter(k => !offKeys.has(k)).map(k => onKeys.get(k));
  const removed = [...offKeys.keys()].filter(k => !onKeys.has(k)).map(k => offKeys.get(k));
  out.push({ id, statusOn: a.status, statusOff: b.status, nOn: a.findings.length, nOff: b.findings.length,
             added, removed, errOn: a.error, errOff: b.error });
}
writeFileSync(join(scratch, "delta.json"), JSON.stringify(out, null, 2));
for (const r of out) {
  if (r.skipped) { console.log(`SKIP ${r.id}: ${r.skipped}`); continue; }
  const flag = (r.added.length || r.removed.length || r.statusOn !== r.statusOff) ? "*" : " ";
  console.log(`${flag} ${r.id.padEnd(52)} ${String(r.statusOff).padEnd(14)}->${String(r.statusOn).padEnd(14)} off=${r.nOff} on=${r.nOn} +${r.added.length} -${r.removed.length}${r.errOn ? " ERRON" : ""}${r.errOff ? " ERROFF" : ""}`);
}
