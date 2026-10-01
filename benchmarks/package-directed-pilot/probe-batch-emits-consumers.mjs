// Test the newly closed identity wrapper through real consumers, including a
// read hidden inside the returned caller object. Observations confer no authority.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { oracleCompilerOptions } from "../../scripts/tsc-oracle.mjs";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const [runFile, prefixArgument, outputArgument] = process.argv.slice(2);
const prefix = resolve(prefixArgument), out = resolve(outputArgument);
assert(!existsSync(out)); mkdirSync(out, { recursive: true });
const read = path => JSON.parse(readFileSync(path));
const hash = bytes => "sha256:" + createHash("sha256").update(bytes).digest("hex");
for (const key of ["SOLID_CHECKER_NATIVE_BIN", "SOLID_TYPEFACTS_BIN", "SOLID_CHECKER_PROBE_NODE"])
  assert(process.env[key] && existsSync(process.env[key]));
assert.equal(read(join(repo, "packages/cli/node_modules/typescript/package.json")).version, "5.9.3");
const retained = read(runFile).results.find(row => row.package === "@solid-primitives/event-bus");
assert.equal(retained.version, "3.0.0-next.3");
for (const name of ["solid-js", "@solidjs/web", "@solidjs/signals"])
  assert.equal(read(join(retained.retainedArtifacts.projectDir, "node_modules", name, "package.json")).version, "2.0.0-rc.9");
const imports = 'import { batchEmits } from "@solid-primitives/event-bus";\n';
const component = imports + 'import { createSignal } from "solid-js";\n';
const specimens = [
  { id: "call", source: imports + "batchEmits({ emit: () => 1 });" },
  { id: "member-call", source: imports + "const bus = batchEmits({ emit: () => 1 }); bus.emit();" },
  { id: "hidden-misuse", source: component + 'export default function App() { const [read] = createSignal(1); const bus = batchEmits({ emit: () => read() }); const value = bus.emit(); return <p>{value}</p>; }' },
  { id: "hidden-correct", source: component + 'export default function App() { const [read] = createSignal(1); const bus = batchEmits({ emit: () => read() }); return <p>{bus.emit()}</p>; }' },
  { id: "direct-misuse", source: component + 'export default function App() { const [read] = createSignal(1); const value = read(); const bus = batchEmits({ emit: () => value }); return <p>{bus.emit()}</p>; }' }
];
const document = { authority: false, kind: "batch-emits-consumers",
  checkerSha256: hash(readFileSync(process.env.SOLID_CHECKER_NATIVE_BIN)), results: [] };
const sourceRoot = join(realpathSync(retained.retainedArtifacts.projectDir), `solid-checker-batch-emits-${hash(out).slice(7, 19)}`);
mkdirSync(sourceRoot);
async function child(args) {
  const process_ = Bun.spawn(args, { cwd: repo, env: { ...process.env, SOLID_CHECKER_DAEMON: "0" }, stdout: "pipe", stderr: "pipe" });
  const [status, stdout, stderr] = await Promise.all([process_.exited,
    new Response(process_.stdout).text(), new Response(process_.stderr).text()]);
  return { status, stdout, stderr };
}
for (const specimen of specimens) {
  const project = join(sourceRoot, specimen.id + ".json");
  writeFileSync(join(sourceRoot, specimen.id + ".tsx"), specimen.source);
  writeFileSync(project, JSON.stringify({ compilerOptions: oracleCompilerOptions("v2", true), files: [specimen.id + ".tsx"] }));
  const tsc = await child([process.env.SOLID_CHECKER_PROBE_NODE, join(repo, "packages/cli/node_modules/typescript/lib/tsc.js"), "--project", project]);
  const observed = { id: specimen.id, tsc, hosts: {} }; document.results.push(observed);
  writeFileSync(join(out, "results.json"), JSON.stringify(document, null, 2));
  assert.deepEqual(tsc, { status: 0, stdout: "", stderr: "" }, specimen.id);
  for (const host of ["none", "browser", "node"]) {
    const census = read(`${prefix}-${host}-1/results.json`);
    assert.equal(census.checkerSha256, document.checkerSha256);
    const row = census.results.find(item => item.package === retained.package);
    assert.equal(row.status, "observed");
    assert(row.surface.some(item => item.entrypoint === "." && item.export === "batchEmits" && item.state === "clean"));
    const dir = join(`${prefix}-${host}-1`, retained.package.replaceAll("/", "__"));
    observed.hosts[host] = {};
    for (const variant of ["baseline", "generated"]) {
      const flags = variant === "generated" ? ["--accepted-contracts", join(dir, "accepted/accepted-contracts.json"),
        "--receipt-trust-configuration", join(dir, "trust.json")] : [];
      const response = await child([process.env.SOLID_CHECKER_NATIVE_BIN, "--format", "json", "--project", project,
        ...(host === "none" ? [] : ["--runtime-target", host]), ...flags]);
      writeFileSync(join(out, `${specimen.id}-${host}-${variant}.json`), JSON.stringify(response, null, 2));
      assert([0, 1].includes(response.status), response.stderr);
      const parsed = JSON.parse(response.stdout);
      if (variant === "generated") assert(parsed.packageSummaries.some(item => item.name === retained.package
        && item.version === retained.version && item.evidence === "accepted"));
      observed.hosts[host][variant] = { status: parsed.status, findings: parsed.findings.map(({ id, rule, kind }) => ({ id, rule, kind })) };
    }
    const findings = observed.hosts[host].generated.findings;
    if (specimen.id === "direct-misuse") assert(findings.some(item => item.id === "SC1001" && item.kind === "violation"));
    if (specimen.id === "hidden-correct") assert(!findings.some(item => item.id === "SC1001" && item.kind === "violation"));
  }
  writeFileSync(join(out, "results.json"), JSON.stringify(document, null, 2) + "\n");
  console.log(JSON.stringify(observed));
}
