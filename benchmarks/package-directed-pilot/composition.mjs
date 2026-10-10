// Offline whole-package baseline. Observations never supply proof authority.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { generatePackageContract } from "../../packages/cli/scripts/generate-package-contract.mjs";
import { certifyContract, certificationImporterPathFor } from "../../packages/cli/scripts/certify-contract.mjs";
import { consumerState } from "../../scripts/contract-coverage-census.mjs";
import { checkComposition } from "./check-composition.mjs";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
assert.equal(JSON.parse(readFileSync(join(repo, "packages/cli/node_modules/typescript/package.json"))).version, "5.9.3",
  "The experiment requires the audited TypeScript release");
const runPath = resolve(process.argv[2]);
const out = resolve(process.argv[3]);
assert(!existsSync(out), "Preserve earlier observations; choose a fresh output directory");
for (const key of ["SOLID_CHECKER_NATIVE_BIN", "SOLID_TYPEFACTS_BIN", "SOLID_CHECKER_PROBE_NODE"]) {
  assert(process.env[key] && existsSync(process.env[key]), `${key} must name an existing pinned binary`);
}
const run = JSON.parse(readFileSync(runPath));
const row = run.results.find(item => item.package === "@solid-primitives/timer");
assert.equal(row?.version, "1.4.5-next.1");
const packageRoot = realpathSync(join(row.retainedArtifacts.projectDir, "node_modules", row.package));
for (const dependency of ["solid-js", "@solidjs/web", "@solidjs/signals"]) {
  assert.equal(JSON.parse(readFileSync(join(dirname(dirname(packageRoot)), dependency, "package.json"))).version, "2.0.0-rc.9");
}
const retained = join(realpathSync(row.retainedArtifacts.outputDir), "@solid-primitives__timer@1.4.5-next.1--solid2--head.json");
const integrity = JSON.parse(readFileSync(retained)).package.integrity;
const digest = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const write = (path, value) => writeFileSync(path, JSON.stringify(value, null, 2) + "\n");
async function child(args) {
  const proc = Bun.spawn(args, { cwd: repo, env: { ...process.env, SOLID_CHECKER_DAEMON: "0" }, stdout: "pipe", stderr: "pipe" });
  const [status, stdout, stderr] = await Promise.all([proc.exited, new Response(proc.stdout).text(), new Response(proc.stderr).text()]);
  return { status, stdout, stderr };
}
mkdirSync(out, { recursive: true });
const document = { authority: false, kind: "composition-baseline", runPath,
  checkerSha256: digest(readFileSync(process.env.SOLID_CHECKER_NATIVE_BIN)),
  producerSha256: digest(readFileSync(process.env.SOLID_TYPEFACTS_BIN)),
  package: row.package, version: row.version, integrity, results: [] };
const cases = [
  { id: "whole-surface", source: 'import { createIntervalCounter, createPolled, createTimeoutLoop, createTimer, makeTimer } from "@solid-primitives/timer";\nconst a = createIntervalCounter(100);\nconst b = createPolled((previous: number) => previous + 1, 100, 0);\ncreateTimeoutLoop(() => {}, 100);\ncreateTimer(() => {}, 100, setInterval);\nconst clear = makeTimer(() => {}, 100, setTimeout);\nexport const values = [a(), b()];\nclear();\n' },
  { id: "untracked-callback", source: 'import { createSignal } from "solid-js";\nimport { createPolled } from "@solid-primitives/timer";\nexport default function App() {\n const [count] = createSignal(0);\n const value = createPolled(() => count(), 100);\n return <p>{String(value())}</p>;\n}\n' },
  { id: "intentional-snapshot", source: 'import { createSignal, untrack } from "solid-js";\nimport { createPolled } from "@solid-primitives/timer";\nexport default function App() {\n const [count] = createSignal(0);\n const value = createPolled(() => untrack(count), 100);\n return <p>{String(value())}</p>;\n}\n' },
  { id: "captured-callable", source: 'import { createSignal } from "solid-js";\nimport { createPolled } from "@solid-primitives/timer";\nexport default function App() {\n const [count] = createSignal(0);\n const captured = createPolled(() => count, 100);\n const value = captured();\n return <p>{String(value())}</p>;\n}\n' }
];
for (const host of ["node", "browser"]) {
  const dir = join(out, host);
  mkdirSync(dir);
  const proposal = join(dir, "proposal.json"), catalog = join(dir, "accepted");
  await generatePackageContract(["--package-root", packageRoot, "--integrity", integrity, "--output", proposal,
    "--host", host, "--entrypoint", ".", "--certification-importer", certificationImporterPathFor({ packageRoot, catalog })]);
  const result = await certifyContract(["--package-root", packageRoot, "--integrity", integrity, "--proposal", proposal,
    "--host", host, "--entrypoint", ".", "--catalog", catalog,
    "--issuer-configuration", retained + ".accepted-catalog.authority/issuer.json",
    "--trust-configuration-output", join(dir, "trust.json"), "--audit-output", join(dir, "audit.json"),
    "--probe-recipe-corpus", join(repo, "scripts/ecosystem-benchmark/probe-recipes")],
    { fetch_: async () => { throw new Error("Offline experiment: exact archive unavailable"); } });
  const audit = JSON.parse(readFileSync(join(dir, "audit.json")));
  assert(result.admitted && audit.ordinaryAnalysis?.receiptAuthenticated && audit.ordinaryAnalysis?.exactCaseSelected);
  assert.equal(audit.graphPreparation?.reusedProposal, true);
  const pointer = JSON.parse(readFileSync(join(catalog, "accepted-contracts.json")));
  const main = JSON.parse(readFileSync(join(catalog, pointer.contracts[0].document)));
  const surface = Object.entries(main.entrypoints["."].cases[0].exports).map(([name, reference]) =>
    ({ export: name, state: consumerState(main, reference) }));
  assert.equal(surface.length, 5);
  const observed = { host, surface, complete: surface.every(item => item.state === "clean"), consumers: [],
    withheldOperations: audit.withheldOperations, withheldClosures: audit.withheldClosures };
  const consumerRoot = join(dirname(dirname(dirname(packageRoot))), `solid-checker-composition-${digest(out).slice(7, 19)}`, host);
  mkdirSync(consumerRoot, { recursive: true });
  for (const item of cases) {
    const source = join(consumerRoot, item.id + ".tsx"), project = join(consumerRoot, item.id + ".json");
    writeFileSync(source, item.source);
    write(project, { compilerOptions: { target: "ES2022", module: "ESNext", moduleResolution: "Bundler", jsx: "preserve",
      jsxImportSource: "@solidjs/web", lib: ["ES2022", "DOM"], types: [], strict: true, skipLibCheck: true, noEmit: true }, files: [item.id + ".tsx"] });
    const tsc = await child([process.env.SOLID_CHECKER_PROBE_NODE, join(repo, "packages/cli/node_modules/typescript/lib/tsc.js"), "--project", project]);
    assert.deepEqual(tsc, { status: 0, stdout: "", stderr: "" }, `Published typings reject ${item.id}`);
    const consumer = { id: item.id, tsc };
    for (const variant of ["baseline", "authored"]) {
      const flags = variant === "authored" ? ["--accepted-contracts", join(catalog, "accepted-contracts.json"),
        "--receipt-trust-configuration", join(dir, "trust.json")] : [];
      const response = await child([process.env.SOLID_CHECKER_NATIVE_BIN, "--format", "json", "--project", project, "--conditions", host, ...flags]);
      write(join(dir, `${item.id}.${variant}.json`), response);
      const parsed = JSON.parse(response.stdout);
      assert(Array.isArray(parsed.findings));
      consumer[variant] = parsed.findings.map(({ id, rule, kind }) => ({ id, rule, kind }));
    }
    observed.consumers.push(consumer);
  }
  document.results.push(observed);
  write(join(out, "results.json"), document);
  console.log(JSON.stringify(observed));
}
// An invocation result need not be plain. A fresh function or object is valid
// under the published generics; preserving it is distinct from invoking again.
const runtime = await child([process.env.SOLID_CHECKER_PROBE_NODE, "--conditions=node", "--input-type=module", "--eval",
  `import { createPolled } from ${JSON.stringify(pathToFileURL(join(packageRoot, "dist/index.js")).href)};
let calls = 0;
const token = () => 42;
const read = createPolled(() => { calls++; return token; }, 100);
const first = read(), second = read();
console.log(JSON.stringify({ calls, same: first === token && second === token, callable: typeof first === "function", value: first() }));`]);
assert.equal(runtime.status, 0, runtime.stderr);
assert.equal(runtime.stderr, "");
document.runtime = JSON.parse(runtime.stdout);
assert.deepEqual(document.runtime, { calls: 1, same: true, callable: true, value: 42 });
const checked = checkComposition(document);
write(join(out, "results.json"), document);
console.log(JSON.stringify({ ...checked, runtime: document.runtime }));
