// Offline, whole-corpus observation. Only the native certification transaction
// can authorize receipts; this runner cannot author or change semantic proposals.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { preparePublishedGraphCases, buildPublishedGraphExecutionRequest,
  certificationImporterPathFor, withheldClosuresFromNativeOutput } from "../../packages/cli/scripts/certify-contract.mjs";
import { catalogSurface, surfaceClosed as closed } from "./catalog-surface.mjs";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const input = resolve(process.argv[2]), out = resolve(process.argv[3]);
const host = process.argv[4];
assert(["none", "browser", "node"].includes(host), "Select one exact host");
assert(!existsSync(out), "Preserve earlier evidence; choose a fresh directory");
for (const key of ["SOLID_CHECKER_NATIVE_BIN", "SOLID_TYPEFACTS_BIN", "SOLID_CHECKER_PROBE_NODE"])
  assert(process.env[key] && existsSync(process.env[key]), `${key} must name a pinned binary`);
const run = JSON.parse(readFileSync(input));
assert.equal(run.results.length, 97, "This experiment measures the pinned 97-package corpus");
assert.equal(new Set(run.results.map(row => row.package)).size, 97);
const measured = JSON.parse(readFileSync(join(dirname(input), host === "none" ? "measure.json" : `measure-${host}.json`)));
const selected = process.argv[5]?.split(",");
if (selected) {
  assert(selected.every(name => run.results.some(row => row.package === `@solid-primitives/${name}`)));
  run.results = run.results.filter(row => selected.includes(row.package.slice("@solid-primitives/".length)));
}
const previous = process.argv[6] ? JSON.parse(readFileSync(resolve(process.argv[6]))) : null;
const hash = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const write = (path, value) => writeFileSync(path, JSON.stringify(value, null, 2) + "\n");
mkdirSync(out, { recursive: true });
const document = { authority: false, kind: "automatic-composition-surface-census", host,
  authoredProposals: 0, input, inputSha256: hash(readFileSync(input)),
  checkerSha256: hash(readFileSync(process.env.SOLID_CHECKER_NATIVE_BIN)),
  producerSha256: hash(readFileSync(process.env.SOLID_TYPEFACTS_BIN)),
  results: run.results.map(row => ({ package: row.package, version: row.version, status: "pending" })) };
const save = () => write(join(out, "results.json"), document);
save();
async function child(args) {
  const process_ = Bun.spawn(args, { cwd: repo, env: { ...process.env, SOLID_CHECKER_DAEMON: "0" }, stdout: "pipe", stderr: "pipe" });
  const [status, stdout, stderr] = await Promise.all([process_.exited,
    new Response(process_.stdout).text(), new Response(process_.stderr).text()]);
  return { status, stdout, stderr };
}
async function observe(index) {
  const row = run.results[index], observed = document.results[index], start = performance.now();
  const dir = join(out, row.package.replaceAll("/", "__")); mkdirSync(dir);
  try {
    const retained = join(row.retainedArtifacts.outputDir,
      `${row.package.replaceAll("/", "__")}@${row.version}--solid2--head.json`);
    if (!existsSync(retained)) throw new Error("Retained root proposal unavailable");
    const earlier = JSON.parse(readFileSync(retained));
    assert.equal(earlier.package.name, row.package); assert.equal(earlier.package.version, row.version);
    // Retained proposals can omit a recovered entrypoint (SSE's main export).
    // The checkpoint's measured surface supplies inventory only, never claims.
    const expected = measured.packages.find(item => item.package === row.package && item.version === row.version);
    assert(expected);
    const already = previous?.results.find(item => item.package === row.package)?.requestedEntrypoints ?? [];
    const entrypoints = [...new Set([...Object.keys(earlier.entrypoints), ...expected.exports.map(item => item.entrypoint)])]
      .filter(entrypoint => !already.includes(entrypoint));
    assert(entrypoints.length > 0, "No retained entrypoint to measure");
    const packageRoot = realpathSync(join(row.retainedArtifacts.projectDir, "node_modules", row.package));
    const catalog = join(dir, "accepted");
    const options = { packageRoot, integrity: earlier.package.integrity, registryOrigin: "https://registry.npmjs.org",
      catalog, issuerConfiguration: retained + ".accepted-catalog.authority/issuer.json",
      host: host === "none" ? null : host, conditions: [], entrypoints,
      probeRecipeCorpus: join(repo, "scripts/ecosystem-benchmark/probe-recipes") };
    observed.requestedEntrypoints = entrypoints;
    observed.integrity = options.integrity;
    observed.status = "preparing"; save();
    const graph = await preparePublishedGraphCases({ options,
      manifest: JSON.parse(readFileSync(join(packageRoot, "package.json"))), scratch: join(dir, "graph"),
      certificationImporter: certificationImporterPathFor({ packageRoot, catalog }),
      dependencyCases: entrypoints.map(entrypoint => ({ entrypoint, conditions: host === "none" ? [] : [host] })),
      fetch_: async () => { throw new Error("Offline census: exact archive unavailable"); } });
    observed.preparationRefusals = graph.timing?.preparationRefusals ?? [];
    if (graph.preparedCases.length === 0) throw new Error(`No exact graph prepared: ${JSON.stringify(graph)}`);
    const states = [...new Map(graph.preparedCases.flatMap(item => item.nodes)
      .map(state => [state.planning.proposal, state])).values()];
    const inputs = states.map(state => ({ package: state.node.packageName, proposal: state.planning.proposal,
      sha256: hash(readFileSync(state.planning.proposal)) }));
    write(join(dir, "generated-inputs.json"), inputs);
    const execution = buildPublishedGraphExecutionRequest({ cases: graph.preparedCases,
      typefactsExecutable: process.env.SOLID_TYPEFACTS_BIN, issuerConfiguration: options.issuerConfiguration,
      catalogRoot: catalog, trustConfigurationOutput: join(dir, "trust.json"), probeHarnessRoot: repo,
      probeNodeExecutable: process.env.SOLID_CHECKER_PROBE_NODE, probeRecipeCorpus: options.probeRecipeCorpus });
    const request = join(dir, "execution.json"); write(request, execution);
    observed.status = "certifying"; save();
    const native = await child([process.env.SOLID_CHECKER_NATIVE_BIN, "--execute-contract-certification", request]);
    write(join(dir, "native.json"), native);
    for (const input of inputs) assert.equal(hash(readFileSync(input.proposal)), input.sha256, "Generated proposal mutated");
    if (native.status !== 0) throw new Error(`Native transaction refused (${native.status}): ${native.stderr}`);
    observed.surface = catalogSurface(catalog, row.package, row.version);
    observed.clean = observed.surface.filter(item => closed(item.state)).length;
    observed.total = observed.surface.length;
    observed.complete = graph.preparedCases.length === entrypoints.length && observed.clean === observed.total;
    observed.withheldClosures = withheldClosuresFromNativeOutput(native.stdout);
    observed.status = "observed";
  } catch (error) {
    observed.status = "refused"; observed.reason = String(error);
  }
  observed.durationMs = Math.round(performance.now() - start); save();
  console.log(JSON.stringify({ package: row.package, host, status: observed.status,
    clean: observed.clean, total: observed.total, complete: observed.complete,
    reason: observed.reason, durationMs: observed.durationMs }));
}
let next = 0;
// Separate native sessions and catalogs; two workers bound memory use. No
// compiler builds run here, and no mutable proof input is shared between roots.
await Promise.all(Array.from({ length: 2 }, async () => {
  while (next < run.results.length) { const index = next++; await observe(index); }
}));
assert(document.results.every(row => ["observed", "refused"].includes(row.status)));
document.completed = true; save();
