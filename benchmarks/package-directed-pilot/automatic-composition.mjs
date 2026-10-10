// Offline breadth experiment. Every semantic proposal comes from the native
// generator; neither observations nor runtime samples carry proof authority.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { preparePublishedGraphCases, buildPublishedGraphExecutionRequest, certificationImporterPathFor,
  withheldClosuresFromNativeOutput, withheldOperationsFromNativeOutput } from "../../packages/cli/scripts/certify-contract.mjs";
import { consumerState } from "../../scripts/contract-coverage-census.mjs";
import { automaticCompositionCases as specs, callTimeProbeSource } from "./automatic-composition-cases.mjs";
import { parameterPassthroughCases } from "./parameter-passthrough-cases.mjs";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const run = JSON.parse(readFileSync(resolve(process.argv[2]))), out = resolve(process.argv[3]);
const requested = process.argv[4]?.split(",");
const hosts = process.argv[5]?.split(",") ?? ["node", "browser"];
assert(hosts.length > 0 && new Set(hosts).size === hosts.length
  && hosts.every(host => ["none", "node", "browser"].includes(host)), "Select exact hosts");
const selectedSpecs = requested ? [...specs, ...parameterPassthroughCases].filter(spec => requested.includes(spec.name)) : specs;
if (requested) assert(requested.length > 0 && requested.every(name => selectedSpecs.some(spec => spec.name === name)),
  "Select known package names separated by commas");
assert(!existsSync(out), "Preserve earlier evidence; choose a fresh directory");
for (const key of ["SOLID_CHECKER_NATIVE_BIN", "SOLID_TYPEFACTS_BIN", "SOLID_CHECKER_PROBE_NODE"])
  assert(process.env[key] && existsSync(process.env[key]), `${key} must name a pinned binary`);
assert.equal(JSON.parse(readFileSync(join(repo, "packages/cli/node_modules/typescript/package.json"))).version, "5.9.3");
const hash = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const write = (path, value) => writeFileSync(path, JSON.stringify(value, null, 2) + "\n");
async function child(args) {
  const process_ = Bun.spawn(args, { cwd: repo, env: { ...process.env, SOLID_CHECKER_DAEMON: "0" }, stdout: "pipe", stderr: "pipe" });
  const [status, stdout, stderr] = await Promise.all([process_.exited, new Response(process_.stdout).text(), new Response(process_.stderr).text()]);
  return { status, stdout, stderr };
}
mkdirSync(out, { recursive: true });
const document = { authority: false, kind: "automatic-composition-breadth", authoredProposals: 0,
  checkerSha256: hash(readFileSync(process.env.SOLID_CHECKER_NATIVE_BIN)), producerSha256: hash(readFileSync(process.env.SOLID_TYPEFACTS_BIN)), results: [] };
for (const spec of selectedSpecs) {
  const row = run.results.find(item => item.package === `@solid-primitives/${spec.name}`);
  assert.equal(row?.version, spec.version);
  const packageRoot = realpathSync(join(row.retainedArtifacts.projectDir, "node_modules", row.package));
  for (const dependency of ["solid-js", "@solidjs/web", "@solidjs/signals"])
    assert.equal(JSON.parse(readFileSync(join(dirname(dirname(packageRoot)), dependency, "package.json"))).version, "2.0.0-rc.9");
  const retained = join(realpathSync(row.retainedArtifacts.outputDir), `@solid-primitives__${spec.name}@${spec.version}--solid2--head.json`);
  const integrity = JSON.parse(readFileSync(retained)).package.integrity;
  for (const host of hosts) {
    const dir = join(out, `${spec.name}-${host}`); mkdirSync(dir);
    const catalog = join(dir, "accepted");
    const options = { packageRoot, integrity, registryOrigin: "https://registry.npmjs.org", catalog,
      issuerConfiguration: retained + ".accepted-catalog.authority/issuer.json", host: host === "none" ? null : host, conditions: [], entrypoints: ["."],
      probeRecipeCorpus: join(repo, "scripts/ecosystem-benchmark/probe-recipes") };
    const observed = { package: row.package, version: row.version, integrity, host, targets: spec.targets, consumers: [] };
    document.results.push(observed);
    let graph;
    try {
      graph = await preparePublishedGraphCases({ options, manifest: JSON.parse(readFileSync(join(packageRoot, "package.json"))),
        scratch: join(dir, "graph"), certificationImporter: certificationImporterPathFor({ packageRoot, catalog }),
        dependencyCases: [{ entrypoint: ".", conditions: host === "none" ? [] : [host] }],
        fetch_: async () => { throw new Error("Offline experiment: exact archive unavailable"); } });
      if (graph.preparedCases.length !== 1) throw new Error(`Exact graph unavailable: ${JSON.stringify(graph)}`);
    } catch (error) {
      observed.refused = { stage: "graph-preparation", reason: String(error) };
      write(join(out, "results.json"), document); console.log(JSON.stringify(observed)); continue;
    }
    const inputs = graph.preparedCases[0].nodes.map(state => ({ package: state.node.packageName,
      proposal: state.planning.proposal, sha256: hash(readFileSync(state.planning.proposal)) }));
    write(join(dir, "generated-inputs.json"), inputs);
    const execution = buildPublishedGraphExecutionRequest({ cases: graph.preparedCases,
      typefactsExecutable: process.env.SOLID_TYPEFACTS_BIN, issuerConfiguration: options.issuerConfiguration,
      catalogRoot: catalog, trustConfigurationOutput: join(dir, "trust.json"), probeHarnessRoot: repo,
      probeNodeExecutable: process.env.SOLID_CHECKER_PROBE_NODE, probeRecipeCorpus: options.probeRecipeCorpus });
    const request = join(dir, "execution.json"); write(request, execution);
    const native = await child([process.env.SOLID_CHECKER_NATIVE_BIN, "--execute-contract-certification", request]);
    write(join(dir, "native.json"), native);
    for (const input of inputs) assert.equal(hash(readFileSync(input.proposal)), input.sha256, "Generated proposal was mutated");
    if (native.status !== 0) {
      observed.refused = { stage: "native-certification", status: native.status, reason: native.stderr };
      write(join(out, "results.json"), document); console.log(JSON.stringify(observed)); continue;
    }
    const pointer = JSON.parse(readFileSync(join(catalog, "accepted-contracts.json")));
    const accepted = JSON.parse(readFileSync(join(catalog, pointer.contracts[0].document)));
    assert.equal(accepted.package.name, row.package);
    assert.equal(accepted.package.version, row.version);
    assert(spec.body !== null, "Known invalid published graph unexpectedly admitted");
    observed.surface = Object.entries(accepted.entrypoints["."].cases[0].exports).map(([name, reference]) =>
      ({ export: name, state: consumerState(accepted, reference) }));
    observed.complete = observed.surface.length > 0 && observed.surface.every(item => item.state === "clean");
    observed.withheldClosures = withheldClosuresFromNativeOutput(native.stdout);
    observed.withheldOperations = withheldOperationsFromNativeOutput(native.stdout);
    const imports = `import { ${spec.targets.join(", ")} } from ${JSON.stringify(row.package)};\ndeclare const element: HTMLElement;\n`;
    const cases = [
      { id: "unowned", source: imports + spec.body },
      { id: "owned", source: imports + 'import { createRoot } from "solid-js";\ncreateRoot(() => {\n' + spec.body + '\n});' },
      { id: "callback", source: imports + 'import { createSignal } from "solid-js";\nconst [read] = createSignal(1);\n' +
        (spec.callbackBody ?? spec.body.replaceAll("() => {}", "() => { read(); }")) }
    ];
    const consumerRoot = join(dirname(dirname(dirname(packageRoot))), `solid-checker-breadth-${hash(out).slice(7, 19)}`, spec.name, host);
    mkdirSync(consumerRoot, { recursive: true });
    for (const specimen of cases) {
      const project = join(consumerRoot, specimen.id + ".json");
      writeFileSync(join(consumerRoot, specimen.id + ".tsx"), specimen.source + "\n");
      write(project, { compilerOptions: { target: "ES2022", module: "ESNext", moduleResolution: "Bundler", jsx: "preserve",
        jsxImportSource: "@solidjs/web", lib: ["ES2022", "DOM"], types: [], strict: true, skipLibCheck: true, noEmit: true }, files: [specimen.id + ".tsx"] });
      const tsc = await child([process.env.SOLID_CHECKER_PROBE_NODE, join(repo, "packages/cli/node_modules/typescript/lib/tsc.js"), "--project", project]);
      const consumer = { id: specimen.id, tsc }; observed.consumers.push(consumer);
      write(join(out, "results.json"), document);
      assert.deepEqual(tsc, { status: 0, stdout: "", stderr: "" }, `Published typings rejected ${spec.name}/${specimen.id}`);
      for (const variant of ["baseline", "generated"]) {
        const flags = variant === "generated" ? ["--accepted-contracts", join(catalog, "accepted-contracts.json"),
          "--receipt-trust-configuration", join(dir, "trust.json")] : [];
        const response = await child([process.env.SOLID_CHECKER_NATIVE_BIN, "--format", "json", "--project", project,
          ...(host === "none" ? [] : ["--runtime-target", host]), ...flags]);
        write(join(dir, `${specimen.id}.${variant}.json`), response);
        assert([0, 1].includes(response.status), response.stderr);
        const parsed = JSON.parse(response.stdout);
        assert(Array.isArray(parsed.findings));
        consumer[variant] = { status: parsed.status, accepted: parsed.packageSummaries?.some(item => item.name === row.package
          && item.version === row.version && item.evidence === "accepted") ?? false,
          findings: parsed.findings.map(({ id, rule, kind }) => ({ id, rule, kind })) };
      }
      assert(consumer.generated.accepted, "Consumer did not admit the generated receipt");
      assert(!consumer.generated.findings.some(item => specimen.id === "owned" && item.id === "SC4001" && item.kind === "violation"));
    }
    if (host === "node") {
      const module = pathToFileURL(join(packageRoot, "dist/index.js")).href;
      const runtime = await child([process.env.SOLID_CHECKER_PROBE_NODE, "--conditions=node", "--input-type=module", "--eval",
        callTimeProbeSource(spec, module)]);
      write(join(dir, "runtime.json"), runtime);
      observed.runtime = runtime.status === 0 ? JSON.parse(runtime.stdout) : { status: runtime.status, stderr: runtime.stderr };
    }
    write(join(out, "results.json"), document);
    console.log(JSON.stringify({ package: observed.package, host, clean: observed.surface.filter(item => item.state === "clean").length,
      total: observed.surface.length, complete: observed.complete, consumers: observed.consumers, runtime: observed.runtime }));
  }
}
write(join(out, "results.json"), document);
