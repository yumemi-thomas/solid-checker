// Offline composition experiment; observations confer no proof authority.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { generatePackageContract } from "../../packages/cli/scripts/generate-package-contract.mjs";
import { preparePublishedGraphCases, buildPublishedGraphExecutionRequest, mergeProposalDependencies, certificationImporterPathFor,
  withheldClosuresFromNativeOutput, withheldOperationsFromNativeOutput } from "../../packages/cli/scripts/certify-contract.mjs";
import { consumerState } from "../../scripts/contract-coverage-census.mjs";
import { checkComposedCursor } from "./check-composed-cursor.mjs";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
assert.equal(JSON.parse(readFileSync(join(repo, "packages/cli/node_modules/typescript/package.json"))).version, "5.9.3",
  "The experiment requires the audited TypeScript release");
const runPath = resolve(process.argv[2]), out = resolve(process.argv[3]);
const openDependency = process.argv[4] === "open-dependency";
const withheldDependency = process.argv[4] === "withheld-dependency";
assert(!process.argv[4] || openDependency || withheldDependency, "Unknown composition mode");
assert(!existsSync(out), "Choose a fresh output directory; preserve earlier evidence");
for (const key of ["SOLID_CHECKER_NATIVE_BIN", "SOLID_TYPEFACTS_BIN", "SOLID_CHECKER_PROBE_NODE"])
  assert(process.env[key] && existsSync(process.env[key]), `${key} must name an existing pinned binary`);
const row = JSON.parse(readFileSync(runPath)).results.find(item => item.package === "@solid-primitives/cursor");
assert.equal(row?.version, "1.0.0-next.2");
const packageRoot = realpathSync(join(row.retainedArtifacts.projectDir, "node_modules", row.package));
for (const dependency of ["solid-js", "@solidjs/web", "@solidjs/signals"])
  assert.equal(JSON.parse(readFileSync(join(dirname(dirname(packageRoot)), dependency, "package.json"))).version, "2.0.0-rc.9");
const retained = join(realpathSync(row.retainedArtifacts.outputDir), "@solid-primitives__cursor@1.0.0-next.2--solid2--head.json");
const integrity = JSON.parse(readFileSync(retained)).package.integrity;
const digest = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const write = (path, value) => writeFileSync(path, JSON.stringify(value, null, 2) + "\n");
async function child(args) {
  const proc = Bun.spawn(args, { cwd: repo, env: { ...process.env, SOLID_CHECKER_DAEMON: "0" }, stdout: "pipe", stderr: "pipe" });
  const [status, stdout, stderr] = await Promise.all([proc.exited, new Response(proc.stdout).text(), new Response(proc.stderr).text()]);
  return { status, stdout, stderr };
}
mkdirSync(out, { recursive: true });
const document = { authority: false, kind: "composed-cursor", openDependency, withheldDependency, package: row.package, version: row.version, integrity,
  checkerSha256: digest(readFileSync(process.env.SOLID_CHECKER_NATIVE_BIN)),
  producerSha256: digest(readFileSync(process.env.SOLID_TYPEFACTS_BIN)), results: [] };
const imports = 'import { createBodyCursor, createDragCursor, createElementCursor, cursorRef, makeBodyCursor, makeElementCursor } from "@solid-primitives/cursor";\n';
const body = 'const a = makeBodyCursor("pointer");\nconst b = makeElementCursor(element, "pointer");\ncreateBodyCursor(() => "pointer");\ncreateElementCursor(element, "pointer");\ncreateDragCursor(element);\ncursorRef("pointer")(element);\na(); b();\n';
const cases = [
  { id: "whole-surface", source: imports + 'declare const element: HTMLElement;\n' + body },
  { id: "owned-surface", source: imports + 'import { createRoot } from "solid-js";\ndeclare const element: HTMLElement;\ncreateRoot(() => {\n' + body + '});\n' },
  { id: "callback-control", source: 'import { createBodyCursor } from "@solid-primitives/cursor";\nimport { createSignal } from "solid-js";\nconst [value] = createSignal("pointer" as const);\ncreateBodyCursor(() => value());\n' }
];
for (const host of openDependency || withheldDependency ? ["node"] : ["node", "browser"]) {
  const dir = join(out, host); mkdirSync(dir);
  const proposal = join(dir, "proposal.json"), catalog = join(dir, "accepted");
  await generatePackageContract(["--package-root", packageRoot, "--integrity", integrity, "--output", proposal,
    "--host", host, "--entrypoint", ".", "--certification-importer", certificationImporterPathFor({ packageRoot, catalog })]);
  const options = { packageRoot, integrity, registryOrigin: "https://registry.npmjs.org", catalog,
    issuerConfiguration: retained + ".accepted-catalog.authority/issuer.json", host, conditions: [], entrypoints: ["."],
    probeRecipeCorpus: join(repo, "scripts/ecosystem-benchmark/probe-recipes") };
  const graph = await preparePublishedGraphCases({ options, manifest: JSON.parse(readFileSync(join(packageRoot, "package.json"))),
    scratch: join(dir, "graph"), certificationImporter: certificationImporterPathFor({ packageRoot, catalog }),
    dependencyCases: [{ entrypoint: ".", conditions: [host] }],
    fetch_: async () => { throw new Error("Offline experiment: exact archive unavailable"); } });
  assert.equal(graph.preparedCases.length, 1, "The exact host graph must survive preparation");
  const graphCase = graph.preparedCases[0];
  for (const state of graphCase.nodes) {
    if (state.node.packageName !== "@solid-primitives/utils") continue;
    const dependency = JSON.parse(readFileSync(state.planning.proposal));
    for (const item of dependency.entrypoints["."].cases) {
      const ref = item.exports.noop;
      const original = dependency.summaries[typeof ref === "string" ? ref : ref.summary];
      const summary = structuredClone(original);
      item.exports.noop = "composition-noop"; dependency.summaries["composition-noop"] = summary;
      summary.call = { callbacks: [], reads: [], creates: [], returns: ["composition-plain-return"],
        operations: [{ id: "composition-plain-return", kind: "return", trigger: { event: "call" },
          at: { event: "call", schedule: "same-stack" }, tracking: "untracked",
          count: { min: 0, max: "many", scope: "call" }, output: "plain" }],
        closed: ["callbacks", "reads", "creates", "returns"], proposedClosures: ["callbacks", "reads", "creates", "returns"] };
      if (openDependency) {
        delete summary.call.callbacks;
        summary.call.closed = summary.call.closed.filter(domain => domain !== "callbacks");
        summary.call.proposedClosures = [...summary.call.closed];
      }
      if (withheldDependency) {
        summary.call.returns = [];
        delete summary.call.operations;
      }
    }
    write(state.planning.proposal, dependency);
    const planPath = join(state.scratch, "authored-review.json");
    const planned = await child([process.env.SOLID_CHECKER_NATIVE_BIN, "--review-contract", state.planning.proposal,
      "--review-output", planPath]);
    assert.equal(planned.status, 0, planned.stderr);
    state.demandPlan = { ...state.demandPlan, candidateSemanticDigest: JSON.parse(readFileSync(planPath)).semanticDigest };
  }
  // Re-emit the parent's resolution against the changed child proposal. Its
  // semantic digest is replayed by Rust; cache metadata cannot authorize it.
  const merged = mergeProposalDependencies(graphCase.root.directDependencies
    .filter(item => !item.state.statesNothing).map(item => ({ ...item.state, viaSpecifier: item.viaSpecifier,
      reexportImporters: [...(graphCase.root.reexportImporters.get(item.viaSpecifier) ?? [])].sort() })), join(dir, "authored-dependencies"));
  const regenerated = await generatePackageContract(["--package-root", packageRoot, "--integrity", integrity,
    "--output", join(dir, "graph-root.json"), "--conditions", host, "--entrypoint", ".",
    "--certification-importer", graphCase.root.node.importer], {
      proposalDependencies: merged.proposalDependencies, proposalDependencyCatalog: merged.catalog,
      privateGraphPreparation: true, exactConditions: [host]
    });
  graphCase.root.planning.proposal = regenerated.output;
  graphCase.root.planning.resolution = regenerated.certificationInputs[0].resolution;
  graphCase.root.planning.inapplicableCases = regenerated.inapplicableCases ?? [];
  const candidate = JSON.parse(readFileSync(graphCase.root.planning.proposal)); write(join(dir, "automatic-proposal.json"), candidate);
  const artifact = candidate.entrypoints["."].cases[0];
  for (const [target, ref] of Object.entries(artifact.exports)) {
    const summary = structuredClone(candidate.summaries[typeof ref === "string" ? ref : ref.summary]);
    artifact.exports[target] = `composition-${target}`; candidate.summaries[artifact.exports[target]] = summary;
    summary.call.callbacks ??= []; summary.call.reads ??= []; summary.call.creates ??= [];
    summary.call.closed = [...new Set([...(summary.call.closed ?? []), "callbacks", "reads", "creates"])];
    summary.call.proposedClosures = [...summary.call.closed];
  }
  for (const target of ["makeBodyCursor", "makeElementCursor", "cursorRef"]) {
    const ref = artifact.exports[target];
    const summary = structuredClone(candidate.summaries[typeof ref === "string" ? ref : ref.summary]);
    artifact.exports[target] = `composition-${target}`; candidate.summaries[artifact.exports[target]] = summary;
    const returned = { id: "composition-return", kind: "return", trigger: { event: "call" },
      at: { event: "call", schedule: "same-stack" }, tracking: "untracked",
      count: { min: 0, max: "many", scope: "call" },
      output: { kind: "described-callable", reads: [],
        returns: withheldDependency && target !== "cursorRef" ? [] : ["plain"], callbacks: [] } };
    summary.call.operations = [...(summary.call.operations ?? []).filter(op => op.kind !== "return"), returned];
    summary.call.returns = [returned.id];
    summary.call.reads ??= [];
    summary.call.creates ??= [];
    summary.call.callbacks ??= [];
    if (target === "cursorRef") summary.call.callbacks = [];
    summary.call.closed = [...new Set([...(summary.call.closed ?? []), "callbacks", "reads", "creates", "returns"])];
    summary.call.proposedClosures = [...summary.call.closed];
  }
  const referenced = new Set(Object.values(candidate.entrypoints).flatMap(entry => entry.cases.flatMap(item =>
    Object.values(item.exports).map(ref => typeof ref === "string" ? ref : ref.summary))));
  for (const id of Object.keys(candidate.summaries)) if (!referenced.has(id)) delete candidate.summaries[id];
  write(proposal, candidate);
  graphCase.root.planning.proposal = proposal;
  const execution = buildPublishedGraphExecutionRequest({ cases: graph.preparedCases,
    typefactsExecutable: process.env.SOLID_TYPEFACTS_BIN, issuerConfiguration: options.issuerConfiguration,
    catalogRoot: catalog, trustConfigurationOutput: join(dir, "trust.json"),
    probeHarnessRoot: repo, probeNodeExecutable: process.env.SOLID_CHECKER_PROBE_NODE,
    probeRecipeCorpus: options.probeRecipeCorpus });
  const request = join(dir, "execution.json"); write(request, execution);
  const result = await child([process.env.SOLID_CHECKER_NATIVE_BIN, "--execute-contract-certification", request]);
  write(join(dir, "execution-result.json"), result);
  assert.equal(result.status, 0, result.stderr);
  const pointer = JSON.parse(readFileSync(join(catalog, "accepted-contracts.json")));
  const main = JSON.parse(readFileSync(join(catalog, pointer.contracts[0].document)));
  assert.equal(main.package.name, row.package);
  const surface = Object.entries(main.entrypoints["."].cases[0].exports).map(([name, reference]) =>
    ({ export: name, state: consumerState(main, reference) }));
  assert.equal(surface.length, 6);
  const observed = { host, surface, complete: surface.every(item => item.state === "clean"), consumers: [],
    graph: graph.timing, nativeResultSha256: digest(result.stdout),
    withheldClosures: withheldClosuresFromNativeOutput(result.stdout), withheldOperations: withheldOperationsFromNativeOutput(result.stdout) };
  const consumerRoot = join(dirname(dirname(dirname(packageRoot))), `solid-checker-cursor-${digest(out).slice(7, 19)}`, host);
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
      const parsed = JSON.parse(response.stdout); assert(Array.isArray(parsed.findings));
      assert([0, 1].includes(response.status), response.stderr);
      consumer[variant + "Status"] = parsed.status;
      consumer[variant + "Accepted"] = parsed.packageSummaries?.some(item => item.name === row.package
        && item.version === row.version && item.evidence === "accepted") ?? false;
      consumer[variant] = parsed.findings.map(({ id, rule, kind }) => ({ id, rule, kind }));
    }
    observed.consumers.push(consumer);
  }
  document.results.push(observed); write(join(out, "results.json"), document);
  console.log(JSON.stringify({ host, surface, complete: observed.complete, consumers: observed.consumers }));
}
const runtime = await child([process.env.SOLID_CHECKER_PROBE_NODE, "--conditions=node", "--input-type=module", "--eval",
  `import * as cursor from ${JSON.stringify(pathToFileURL(join(packageRoot, "dist/index.js")).href)};
let calls = 0;
const trap = new Proxy({}, {get(){throw new Error("DOM accessed on node");}});
const fn = () => {calls++; throw new Error("callback invoked on node");};
cursor.makeBodyCursor("pointer")(); cursor.makeElementCursor(trap,"pointer")();
cursor.createBodyCursor(fn); cursor.createElementCursor(fn,fn); cursor.createDragCursor(fn);
const ref = cursor.cursorRef(fn); const value = ref(trap);
console.log(JSON.stringify({calls, returnedUndefined:value === undefined}));`]);
assert.equal(runtime.status, 0, runtime.stderr); document.runtime = JSON.parse(runtime.stdout);
assert.deepEqual(document.runtime, { calls: 0, returnedUndefined: true });
write(join(out, "results.json"), document);
console.log(JSON.stringify(checkComposedCursor(document)));
