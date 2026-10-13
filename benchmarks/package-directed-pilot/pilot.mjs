// An offline experiment, not a producer of accepted-tier artifacts.
// All handwritten semantics remain untrusted until the existing Rust verifier
// proves them. An authenticated partial receipt is not package certification.
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { generatePackageContract } from "../../packages/cli/scripts/generate-package-contract.mjs";
import { certifyContract, certificationImporterPathFor } from "../../packages/cli/scripts/certify-contract.mjs";
import { checkExtended } from "./check-extended.mjs";
import { checkCompletion } from "./check-completion.mjs";
import { checkCover, checkBounds } from "./check-cover.mjs";
import { consumerState } from "../../scripts/contract-coverage-census.mjs";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const runPath = resolve(process.argv[2] ?? join(repo, "rust/target/primitives-checkpoint/run.json"));
const out = resolve(process.argv[3] ?? join(repo, "rust/target/package-directed-pilot-replay"));
const run = JSON.parse(readFileSync(runPath));
const ledger = JSON.parse(readFileSync(join(repo, "fixtures/primitives-misuse/cases.json")));
const extended = process.argv[4] === "extended";
const completion = process.argv[4] === "completion";
const cover = process.argv[4] === "cover";
const bounds = process.argv[4] === "bounds";
if (process.argv[4] && !extended && !completion && !cover && !bounds) throw new Error("The optional experiment mode must be extended, completion, cover or bounds");
for (const key of ["SOLID_CHECKER_NATIVE_BIN", "SOLID_TYPEFACTS_BIN", "SOLID_CHECKER_PROBE_NODE"]) {
  if (!process.env[key] || !existsSync(process.env[key])) throw new Error(`${key} must name the existing pinned binary`);
}
if (existsSync(out)) throw new Error("Choose a fresh output directory; prior evidence is preserved");
mkdirSync(out, { recursive: true });
const digest = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const write = (path, value) => writeFileSync(path, JSON.stringify(value, null, 2) + "\n");
const operation = (id, kind) => ({ id, kind, at: { event: "call", schedule: "same-stack" },
  trigger: { event: "call" }, tracking: "untracked", count: { min: 1, max: "many", scope: "call" } });
async function child(args) {
  const proc = Bun.spawn(args, { cwd: repo, env: { ...process.env, SOLID_CHECKER_DAEMON: "0" }, stdout: "pipe", stderr: "pipe" });
  const [status, stdout, stderr] = await Promise.all([proc.exited, new Response(proc.stdout).text(), new Response(proc.stderr).text()]);
  return { status, stdout, stderr };
}
const results = [];
const initialTrials = [
  ["utils", "access", ["utils-access-argument-read", "utils-createMicrotask-module-scope"]],
  ["event-listener", "createEventListener", ["event-listener-createEventListener-module-scope"]],
  ["raf", "createRAF", ["raf-createRAF-module-scope", "raf-createRAF-top-level-read"]],
  ["utils", "access", []] // Negative control: omit the callable length getter's invocation.
];
const extendedTrials = [
  ["event-listener", "createEventListener", ["event-listener-createEventListener-module-scope"], "browser", "owner-possible"],
  ["event-listener", "createEventListener", ["event-listener-createEventListener-module-scope"], "node", "owner-possible"],
  ["event-listener", "createEventListener", ["event-listener-createEventListener-module-scope"], "none", "owner-possible"],
  ["raf", "createRAF", ["raf-createRAF-module-scope"], "browser", "owner-only"],
  ["raf", "createRAF", ["raf-createRAF-module-scope"], "node", "owner-only"],
  ["raf", "createRAF", ["raf-createRAF-module-scope"], "none", "owner-only"],
  ["raf", "createRAF", ["raf-createRAF-top-level-read"], "browser", "return-possible"],
  ["memo", "createPureReaction", ["memo-createPureReaction-module-scope"], "browser", "owner-only"]
];
const completionTrials = [
  ["event-dispatcher", "createEventDispatcher", ["dispatcher-completion"], "node", "complete"],
  ["event-dispatcher", "createEventDispatcher", ["dispatcher-completion"], "browser", "complete"],
  ["platform", "isAndroid", ["platform-completion"], "node", "complete"]
];
const coverTrials = [
  ["event-listener", "createEventListener", ["event-listener-createEventListener-module-scope"], "browser", "owner-guaranteed"],
  ["event-listener", "createEventListener", ["event-listener-createEventListener-module-scope"], "node", "owner-guaranteed"],
  ["event-listener", "createEventListener", ["event-listener-createEventListener-module-scope"], "none", "owner-guaranteed"],
  ["raf", "createRAF", ["raf-createRAF-module-scope"], "browser", "owner-only"],
  ["memo", "createPureReaction", ["memo-createPureReaction-module-scope"], "browser", "owner-only"]
];
const boundsTrials = [
  ["lifecycle", "onElementConnect", ["lifecycle-connect-control"], "browser", "owner-bounds"],
  ["permission", "createPermission", ["permission-createPermission-module-scope"], "browser", "owner-bounds"],
  ["sensors", "createSensor", [], "browser", "owner-bounds"],
  ["timer", "createTimer", ["timer-createTimer-module-scope", "timer-createTimeoutLoop-module-scope"], "browser", "owner-bounds"],
  ["workers", "createReactiveWorker", [], "browser", "owner-bounds"]
];
for (const [name, target, cases, host = "browser", mode = "initial"] of bounds ? boundsTrials : cover ? coverTrials : completion ? completionTrials : extended ? extendedTrials : initialTrials) {
  const negative = name === "utils" && cases.length === 0;
  const trial = extended || completion || cover || bounds ? `${name}-${host}-${mode}` : negative ? "utils-omitted-getter" : name;
  const hostArgs = host === "none" ? [] : ["--host", host];
  const dir = join(out, trial);
  mkdirSync(dir);
  const row = run.results.find(row => row.package === `@solid-primitives/${name}`);
  if (!row?.retainedArtifacts) throw new Error(`No retained published install for ${name}`);
  const packageRoot = realpathSync(join(row.retainedArtifacts.projectDir, "node_modules", row.package));
  const expectedVersion = { utils: "7.0.0-next.4", "event-listener": "3.0.0-next.5", raf: "4.0.0-next.2", memo: "2.0.0-next.2", "event-dispatcher": "1.0.0-next.2", platform: "1.0.0-next.2", lifecycle: "1.0.0-next.2", permission: "2.0.0-next.2", sensors: "1.0.0-next.3", timer: "1.4.5-next.1", workers: "2.0.1-next.1" }[name];
  if (row.version !== expectedVersion) throw new Error(`Unaudited package version: ${row.version}`);
  for (const dependency of ["solid-js", "@solidjs/web", "@solidjs/signals"]) {
    const manifest = JSON.parse(readFileSync(join(dirname(dirname(packageRoot)), dependency, "package.json")));
    if (manifest.version !== "2.0.0-rc.9") throw new Error(`Unaudited runtime: ${dependency}@${manifest.version}`);
  }
  const retained = join(realpathSync(row.retainedArtifacts.outputDir), `@solid-primitives__${name}@${row.version}--solid2--head.json`);
  const integrity = JSON.parse(readFileSync(retained)).package.integrity;
  const proposal = join(dir, "proposal.json"), catalog = join(dir, "accepted");
  await generatePackageContract(["--package-root", packageRoot, "--integrity", integrity, "--output", proposal,
    ...hostArgs, "--entrypoint", ".", "--certification-importer", certificationImporterPathFor({ packageRoot, catalog })]);
  const document = JSON.parse(readFileSync(proposal));
  write(join(dir, "automatic-proposal.json"), document);
  const artifactCase = document.entrypoints["."].cases[0];
  const summary = structuredClone(document.summaries[artifactCase.exports[target]]);
  // Summary references may be shared by unrelated exports. Detach this exact
  // export; changing a shared summary would silently propose extra claims.
  artifactCase.exports[target] = `pilot-${target}`;
  document.summaries[artifactCase.exports[target]] = summary;
  const references = new Set(Object.values(document.entrypoints).flatMap(entry => entry.cases.flatMap(c =>
    Object.values(c.exports).map(ref => typeof ref === "string" ? ref : ref.summary))));
  for (const id of Object.keys(document.summaries)) if (!references.has(id)) delete document.summaries[id];
  const proposedOwnerBounds = [];
  if (bounds) {
    for (const [exportName, reference] of Object.entries(artifactCase.exports)) {
      const value = document.summaries[typeof reference === "string" ? reference : reference.summary];
      for (const op of value.call?.operations ?? []) {
        if (op.owner?.requires === "required" && [0, 1].includes(op.count?.min)) {
          proposedOwnerBounds.push({ export: exportName, operation: op.id, originalMin: op.count.min, proposedMin: 1 });
        }
      }
    }
    for (const value of Object.values(document.summaries)) {
      for (const op of value.call?.operations ?? []) {
        if (op.owner?.requires === "required" && op.count?.min === 0) op.count.min = 1;
      }
    }
  }
  if (negative) {
    summary.call.operations = summary.call.operations.filter(op => op.id !== "callback-1");
    summary.call.callbacks = summary.call.callbacks.filter(item => item.operation !== "callback-1");
  } else if (name === "event-listener") {
    const effect = operation("pilot-effect-owner", "compute");
    if (mode === "owner-possible") effect.count.min = 0;
    effect.owner = { source: "ambient-at-call", requires: "required", requiresChildren: "required" };
    summary.call.operations = [effect];
    summary.call.computations = [effect.id];
  } else if (name === "raf") {
    const ret = operation("pilot-return", "return");
    if (mode === "return-possible") ret.count.min = 0;
    ret.output = { kind: "tuple", closed: ["items"], items: [{ kind: "reactive", role: "accessor" }, "callable", "callable"] };
    const cleanup = operation("pilot-cleanup", "cleanup");
    cleanup.owner = { source: "ambient-at-call", requires: "required", requiresCleanup: "required" };
    summary.call = { operations: [ret, cleanup], returns: [ret.id], cleanups: [cleanup.id], closed: ["returns"], proposedClosures: ["returns"] };
    if (mode === "owner-only") summary.call = { operations: [cleanup], cleanups: [cleanup.id] };
  } else if (name === "memo") {
    const cleanup = operation("pilot-cleanup", "cleanup");
    cleanup.owner = { source: "ambient-at-call", requires: "required", requiresCleanup: "required" };
    // Ownership alone: claims about the returned reaction and its delayed
    // callbacks are deliberately still open.
    summary.call = { operations: [cleanup], cleanups: [cleanup.id] };
  } else if (name === "event-dispatcher") {
    const ret = operation("pilot-return", "return");
    ret.count.min = 0;
    ret.output = { kind: "described-callable", reads: [], returns: ["plain"] };
    summary.call = { operations: [ret], returns: [ret.id], callbacks: [], reads: [], creates: [],
      closed: ["callbacks", "reads", "creates", "returns"], proposedClosures: ["callbacks", "reads", "creates", "returns"] };
  }
  // utils positive control preserves both source-supported invocations and
  // min:0. Neither callable input nor its length implies unconditional calling.
  write(proposal, document);
  const inputs = JSON.parse(readFileSync(proposal + ".certification-inputs.json"));
  inputs.document.sha256 = digest(readFileSync(proposal));
  // This is a cache-reuse envelope, not authority. The proposal-plan sidecar
  // describes the original generator run; native execution derives fresh
  // semantic demands from the authored document, never from that audit plan.
  write(proposal + ".certification-inputs.json", inputs);
  const result = await certifyContract(["--package-root", packageRoot, "--integrity", integrity, "--proposal", proposal,
    ...hostArgs, "--entrypoint", ".", "--catalog", catalog,
    "--issuer-configuration", retained + ".accepted-catalog.authority/issuer.json",
    "--trust-configuration-output", join(dir, "trust.json"), "--audit-output", join(dir, "audit.json"),
    "--probe-recipe-corpus", join(repo, "scripts/ecosystem-benchmark/probe-recipes")],
    { fetch_: async () => { throw new Error("Offline pilot: required archive is not cached"); } });
  const audit = JSON.parse(readFileSync(join(dir, "audit.json")));
  if (audit.graphPreparation?.reusedProposal !== true) throw new Error("Authored proposal was regenerated; the trial is invalid");
  if (!result.admitted || !audit.ordinaryAnalysis?.receiptAuthenticated || !audit.ordinaryAnalysis?.exactCaseSelected)
    throw new Error("Trial receipt did not authenticate and admit under its exact environment");
  const pointer = JSON.parse(readFileSync(join(catalog, "accepted-contracts.json")));
  const main = JSON.parse(readFileSync(join(catalog, pointer.contracts[0].document)));
  const acceptedCase = main.entrypoints["."].cases[0];
  const acceptedSummary = main.summaries[acceptedCase.exports[target]];
  const observed = { trial, package: row.package, version: row.version, host, mode, result,
    packageRoot, integrity, artifact: artifactCase.artifact, declarations: artifactCase.declarations,
    acceptedSummary, withheldClosures: audit.withheldClosures.filter(item => bounds || item.export === target),
    withheldOperations: audit.withheldOperations.filter(item => bounds || item.export === target), consumers: [] };
  if (bounds) {
    observed.ownerBounds = proposedOwnerBounds.map(item => {
      const reference = acceptedCase.exports[item.export];
      const value = main.summaries[typeof reference === "string" ? reference : reference.summary];
      return { ...item, acceptedMin: value.call?.operations?.find(op => op.id === item.operation)?.count?.min ?? null };
    });
  }
  observed.surface = Object.entries(acceptedCase.exports).map(([name, reference]) => ({ export: name, state: consumerState(main, reference) }));
  for (const id of cases) {
    let item = ledger.cases.find(item => item.id === id);
    if (id === "dispatcher-completion") {
      item = { rule: null, kind: null,
        misuse: 'import { createSignal } from "solid-js";\nimport { createEventDispatcher } from "@solid-primitives/event-dispatcher";\nexport default function App() {\n  const [count] = createSignal(0);\n  const dispatch = createEventDispatcher({ onPing: (_event: CustomEvent<number>) => count() });\n  const result = dispatch("ping", 1);\n  return <p>{String(result)}</p>;\n}\n',
        correct: 'import { createEventDispatcher } from "@solid-primitives/event-dispatcher";\nexport default function App() {\n  const dispatch = createEventDispatcher({ onPing: (_event: CustomEvent<number>) => {} });\n  return <p>{String(dispatch("ping", 1))}</p>;\n}\n' };
    } else if (id === "lifecycle-connect-control") {
      const call = 'onElementConnect(document.createElement("div"), () => {});';
      item = { rule: "missing-owner", kind: "uncertifiable",
        misuse: `import { onElementConnect } from "@solid-primitives/lifecycle";\n${call}\n`,
        correct: `import { createRoot } from "solid-js";\nimport { onElementConnect } from "@solid-primitives/lifecycle";\ncreateRoot(dispose => { ${call} return dispose; });\n` };
    } else if (id === "platform-completion") {
      const exports = Object.keys(artifactCase.exports).sort();
      const source = `import { ${exports.join(", ")} } from "@solid-primitives/platform";\nexport default function App() { return <p>{[${exports.join(", ")}].map(String).join(",")}</p>; }\n`;
      item = { rule: null, kind: null, misuse: source, correct: source };
    }
    const consumerRoot = join(dirname(dirname(dirname(packageRoot))), `solid-checker-package-directed-${digest(out).slice(7, 19)}`, trial, id);
    mkdirSync(consumerRoot, { recursive: true });
    const pair = { id, expectedRule: item.rule, expectedKind: item.rule ? item.kind ?? "violation" : null, tsc: {}, baseline: {}, authored: {} };
    for (const part of ["misuse", "correct"]) {
      writeFileSync(join(consumerRoot, part + ".tsx"), item[part]);
      const project = join(consumerRoot, `tsconfig.${part}.json`);
      write(project, { compilerOptions: { target: "ES2022", module: "ESNext", moduleResolution: "Bundler",
        jsx: "preserve", jsxImportSource: "@solidjs/web", lib: ["ES2022", "DOM"], types: [], strict: true,
        skipLibCheck: true, noEmit: true }, files: [part + ".tsx"] });
      pair.tsc[part] = await child([process.env.SOLID_CHECKER_PROBE_NODE, join(repo, "packages/cli/node_modules/typescript/lib/tsc.js"), "--project", project]);
      if (pair.tsc[part].status !== 0) throw new Error(`Published typings reject ${id}/${part}; no checker comparison is permissible`);
      for (const variant of ["baseline", "authored"]) {
        const flags = variant === "authored" ? ["--accepted-contracts", join(catalog, "accepted-contracts.json"), "--receipt-trust-configuration", join(dir, "trust.json")] : [];
        const conditions = host === "none" ? [] : ["--conditions", host];
        const response = await child([process.env.SOLID_CHECKER_NATIVE_BIN, "--format", "json", "--project", project, ...conditions, ...flags]);
        write(join(dir, `${id}.${variant}.${part}.json`), response);
        const parsed = JSON.parse(response.stdout);
        if (!Array.isArray(parsed.findings)) throw new Error("Consumer analysis returned no finding census");
        pair[variant][part] = parsed.findings.map(({ id, rule, kind }) => ({ id, rule, kind }));
      }
    }
    observed.consumers.push(pair);
  }
  if (negative && acceptedSummary.call?.closed?.includes("callbacks")) throw new Error("Incomplete callback proposal incorrectly closed callbacks");
  const contains = (findings, id, kind) => findings.some(f => f.id === id && f.kind === kind);
  if (!negative && name === "utils") {
    for (const pair of observed.consumers) {
      if (JSON.stringify(pair.baseline) !== JSON.stringify(pair.authored) || pair.authored.correct.length !== 0)
        throw new Error("Utility positive control changed findings");
      const code = pair.id.includes("access-") ? "SC1001" : "SC4001";
      if (!contains(pair.authored.misuse, code, pair.expectedKind)) throw new Error("Utility control lost its expected finding");
    }
  }
  if (!extended && name === "raf") {
    const pair = observed.consumers.find(c => c.id === "raf-createRAF-module-scope");
    if (contains(pair.baseline.misuse, "SC4001", "violation") || !contains(pair.authored.misuse, "SC4001", "violation") ||
        contains(pair.authored.correct, "SC4001", "violation")) throw new Error("RAF owner claim did not produce the measured improvement");
    if (acceptedSummary.call?.operations?.some(op => op.id === "pilot-return")) throw new Error("Unsupported RAF tuple was accepted");
  }
  if (!extended && !cover && name === "event-listener" && acceptedSummary.call?.operations?.some(op => op.id === "pilot-effect-owner")) {
    const pair = observed.consumers[0];
    if (!contains(pair.authored.misuse, "SC4001", "violation") || pair.authored.correct.some(item => item.id === "SC4001"))
      throw new Error("Accepted listener registration did not project the reviewed owner finding");
  }
  results.push(observed);
  write(join(out, "results.json"), { mode: bounds ? "bounds" : cover ? "cover" : completion ? "completion" : extended ? "extended" : "initial", runPath, results });
  console.log(JSON.stringify({ trial, status: result.status, withheld: observed.withheldOperations.length,
    consumers: observed.consumers.map(pair => ({ id: pair.id, baseline: pair.baseline, authored: pair.authored })) }));
}
if (extended) console.log(JSON.stringify(checkExtended({ mode: "extended", results })));
if (completion) console.log(JSON.stringify(checkCompletion({ mode: "completion", results })));
if (cover) console.log(JSON.stringify(checkCover({ mode: "cover", results })));
if (bounds) console.log(JSON.stringify(checkBounds({ mode: "bounds", results })));
