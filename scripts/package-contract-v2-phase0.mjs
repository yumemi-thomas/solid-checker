#!/usr/bin/env bun

import { createHash } from "node:crypto";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const FROZEN_BASELINE_JSON_SHA256 = "ea8041adc5872fa63f85f18231330e9b6c0c1232c20f23964e6272143a891f18";
const FROZEN_BASELINE_MARKDOWN_SHA256 = "098bd0cd2a91351019751280b5a95f70406181d9a8da6e0be0072f3ba7332757";

const CONTRACTS = [
  "pkg/contracts/bundled/solid-v1/solid-primitives-debounce.json",
  "pkg/contracts/bundled/solid-v1/solid-primitives-rootless.json",
  "pkg/contracts/bundled/solid-v1/solid-primitives-scheduled.json",
  "pkg/contracts/bundled/solid-v1/solid-js.json",
  "pkg/contracts/bundled/solid-v2/solid-js.json",
  "pkg/contracts/bundled/solid-v2/solidjs-signals.json",
  "pkg/contracts/bundled/solid-v2/solidjs-web.json"
];

const FROZEN_FIXTURES = {
  "unresolved-callee-callback": "false-negative guard: unresolved callback reachability",
  "conditional-export-absence": "false-negative guard: export absent in one artifact condition",
  "conditional-returns-divergence-both": "false-negative guard: condition-dependent return semantics",
  "conditional-callback-conflict": "false-negative guard: incompatible conditional callback behavior",
  "declaration-sibling-reach": "false-negative guard: declaration sibling reachability",
  "escaping-private-helper": "false-negative guard: escaping private callback helper",
  "unreached-private-obligation": "false-negative guard: private obligation outside export reach",
  "class-expression-kind": "over-refusal guard: class expression export kind",
  "function-supertype-kind": "over-refusal guard: callable supertype export kind",
  "torture-environment-conditions": "over-refusal guard: ordered environment conditions",
  "attested-record-matches-walk": "over-refusal guard: matching attested and walked closure",
  "non-literal-dynamic-import": "fail-closed frontier: nonliteral dynamic import",
  "torture-runtime-namespace": "over-refusal guard: runtime namespace export identity"
};

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function read(relativePath) {
  return readFileSync(resolve(root, relativePath));
}

function json(relativePath) {
  return JSON.parse(read(relativePath));
}

function round(value, digits = 3) {
  const scale = 10 ** digits;
  return Math.round(value * scale) / scale;
}

export function measureContract(relativePath) {
  const frozen = json("benchmarks/package-contract-v2/phase0/baseline.json")
    .legacyContracts.contracts.find(contract => contract.path === relativePath);
  if (!frozen) throw new Error(`${relativePath} is not part of the frozen Phase 0 corpus`);
  return structuredClone(frozen);
}

function listFiles(directory) {
  const output = [];
  const walk = current => {
    for (const entry of readdirSync(current, { withFileTypes: true }).sort((a, b) =>
      a.name.localeCompare(b.name)
    )) {
      const path = resolve(current, entry.name);
      if (entry.isDirectory()) walk(path);
      else if (entry.isFile()) output.push(path);
      else throw new Error(`fixture freeze refuses non-file ${relative(root, path)}`);
    }
  };
  walk(directory);
  return output;
}

export function freezeFixture(name, purpose) {
  const directory = resolve(root, "fixtures/package-contracts", name);
  const files = listFiles(directory).map(path => ({
    path: relative(root, path),
    bytes: statSync(path).size,
    sha256: sha256(readFileSync(path))
  }));
  const digestInput = files.map(file => `${file.sha256}  ${file.path}\n`).join("");
  return {
    name,
    purpose,
    treeSha256: sha256(digestInput),
    bytes: files.reduce((total, file) => total + file.bytes, 0),
    files
  };
}

function classifyGenerationFailure(row, generationResult) {
  const common = {
    probeId: row.probeId,
    outcome: "generate-failure",
    failureClass: row.class,
    observedGenerationSignature: generationResult?.signature ?? null
  };
  if (["no-esm-runtime-target", "cjs-only-entrypoint", "no-exported-surface"].includes(row.class)) {
    return { ...common, owner: "resolver", secondaryOwners: [], reason: `runtime export resolution stopped at ${row.class}` };
  }
  if (row.class === "export-kind-unresolved") {
    return {
      ...common,
      owner: "type-facts",
      secondaryOwners: ["schema"],
      reason: "Type Facts supplied no closed runtime-kind answer; schema v1 cannot retain an unknown export kind"
    };
  }
  if (row.class === "export-kind-conflict") {
    return {
      ...common,
      owner: "generator",
      secondaryOwners: ["type-facts"],
      reason: "generator combined a value-kind export with function-only effects"
    };
  }
  throw new Error(`unclassified generation failure ${row.probeId}: ${row.class}`);
}

function classifyRefusal(row) {
  const owner = {
    "kind-observed": "schema",
    "closure-note": "resolver",
    "attested-closure-note": "resolver",
    incompleteness: "probe"
  }[row.rootCause];
  if (!owner) throw new Error(`unclassified refusal ${row.probeId}: ${row.rootCause}`);
  const reason = {
    "kind-observed": "schema v1 cannot encode an unknown export kind, so an unobserved kind cannot be promoted",
    "closure-note": "the walked runtime/declaration closure contains an open frontier",
    "attested-closure-note": "the attested runtime/declaration closure contains an open frontier",
    incompleteness: "the probe/evidence workflow left required claims undriven, unwritten, or otherwise incomplete"
  }[row.rootCause];
  return {
    probeId: row.probeId,
    outcome: "refused",
    owner,
    secondaryOwners: [],
    failureClass: row.rootCause,
    reason,
    preciseBlocker: row.firstBlocker,
    blockerClasses: row.blockerClasses,
    claims: row.claims
  };
}

export function classifyVerificationRows(verification, generation) {
  const byProbe = new Map(generation.results.map(row => [row.probeId, row]));
  const classified = new Map();
  const insert = row => {
    if (classified.has(row.probeId)) throw new Error(`duplicate verification row ${row.probeId}`);
    classified.set(row.probeId, row);
  };
  for (const row of verification.verified) {
    insert({
      probeId: row.probeId,
      outcome: "verified",
      owner: "none",
      secondaryOwners: [],
      failureClass: null,
      reason: `${row.claims.passed}/${row.claims.claims} required claims were driven and passed with no verifier blocker`,
      claims: row.claims
    });
  }
  for (const row of verification.refusals) insert(classifyRefusal(row));
  for (const row of verification.preContractFailures.generateFailures) {
    insert(classifyGenerationFailure(row, byProbe.get(row.probeId)));
  }
  for (const row of verification.preContractFailures.noRuntime) {
    insert({
      probeId: row.probeId,
      outcome: "no-runtime",
      owner: "resolver",
      secondaryOwners: ["runtime"],
      failureClass: "no-runtime",
      reason: row.detail
    });
  }
  const unsupported = [
    ...(verification.preContractFailures.installFailures ?? []),
    ...(verification.preContractFailures.timeouts ?? []),
    ...Object.values(verification.preContractFailures.probeErrors ?? {})
  ];
  if (unsupported.length) {
    throw new Error(`baseline classifier has ${unsupported.length} unsupported pre-contract failures`);
  }
  const expected = verification.overall.rows;
  if (classified.size !== expected) {
    throw new Error(`classified ${classified.size} verification rows, expected ${expected}`);
  }
  return [...classified.values()].sort((left, right) => left.probeId.localeCompare(right.probeId));
}

function summarizeClassifications(rows) {
  const byOutcome = {};
  const byOwner = {};
  const byFailureClass = {};
  for (const row of rows) {
    byOutcome[row.outcome] = (byOutcome[row.outcome] ?? 0) + 1;
    byOwner[row.owner] = (byOwner[row.owner] ?? 0) + 1;
    if (row.failureClass) byFailureClass[row.failureClass] = (byFailureClass[row.failureClass] ?? 0) + 1;
  }
  for (const owner of ["none", "schema", "type-facts", "compiler-facts", "generator", "resolver", "probe", "runtime", "typescript"]) {
    byOwner[owner] ??= 0;
  }
  return { byOutcome, byOwner, byFailureClass };
}

function classifyGenerationAnomalies(generation) {
  return generation.results
    .filter(row => row.outcome === "failure")
    .map(row => {
      if (row.class === "timeout") {
        return {
          probeId: row.probeId,
          outcome: row.outcome,
          failureClass: row.class,
          owner: "runtime",
          secondaryOwners: ["resolver"],
          reason: row.signature
        };
      }
      return classifyGenerationFailure({ probeId: row.probeId, class: row.class }, row);
    });
}

function pinFromCargo(name, cargo, required = true) {
  const expression = new RegExp(`${name.replace(/[.*+?^${}()|[\\]\\]/g, "\\$&")}\\s*=\\s*\\{[^\\n]*rev\\s*=\\s*"([0-9a-f]+)"`);
  const match = cargo.match(expression);
  if (match) return match[1];
  if (required) throw new Error(`could not read ${name} pin from rust/Cargo.toml`);
  return null;
}

export function typeFactsIdentityFromCargo(cargo, buildInfoRaw) {
  const revision = pinFromCargo("typefacts", cargo, false);
  if (revision) return revision;
  if (!/typefacts\s*=\s*\{[^\n]*path\s*=/.test(cargo)) {
    throw new Error("could not read typefacts dependency identity from rust/Cargo.toml");
  }
  let buildInfo;
  try {
    buildInfo = JSON.parse(buildInfoRaw);
  } catch {
    throw new Error("local Type Facts buildinfo is not JSON");
  }
  if (!/^[0-9a-f]{64}$/.test(buildInfo.sourceDigest ?? "")) {
    throw new Error("local Type Facts buildinfo has no source-manifest digest");
  }
  return `source-manifest-sha256:${buildInfo.sourceDigest}`;
}

export function assertSuccessfulCacheDisabledMeasurements(measurements) {
  for (const [name, measurement] of Object.entries(measurements)) {
    if (measurement.exitCode !== 0 || !measurement.command.includes("SOLID_CHECKER_GATE_CACHE=0")) {
      throw new Error(`${name} is not a successful cache-disabled measurement`);
    }
  }
}

function assertBaseline(condition, message) {
  if (!condition) throw new Error(`invalid frozen Phase 0 baseline: ${message}`);
}

function sameJson(left, right) {
  return JSON.stringify(left) === JSON.stringify(right);
}

export function assertFrozenBaseline(report) {
  assertBaseline(report.schemaVersion === 1, "schemaVersion must be 1");
  assertBaseline(
    report.documentKind === "solid-checker-package-contract-phase0-baseline",
    "documentKind is wrong"
  );
  assertBaseline(report.scope?.phase === 0, "scope must remain Phase 0");
  assertBaseline(Number.isFinite(Date.parse(report.capturedAt)), "capturedAt is not an instant");
  assertBaseline(/^[0-9a-f]{40}$/.test(report.repository?.head ?? ""), "repository head is not a commit");

  assertSuccessfulCacheDisabledMeasurements(report.measurements);

  const rows = report.ecosystem?.classifications?.rows ?? [];
  assertBaseline(rows.length === report.ecosystem?.verification?.rows, "classification row count drifted");
  assertBaseline(new Set(rows.map(row => row.probeId)).size === rows.length, "classification IDs are not unique");
  assertBaseline(
    sameJson(summarizeClassifications(rows), report.ecosystem.classifications.summary),
    "classification summary does not match its rows"
  );

  const fixtures = report.fixtureFreeze?.fixtures ?? [];
  assertBaseline(fixtures.length === report.fixtureFreeze?.fixtureCount, "fixture count drifted");
  assertBaseline(
    sameJson(
      fixtures.map(fixture => fixture.name).sort(),
      Object.keys(FROZEN_FIXTURES).sort()
    ),
    "fixture set drifted"
  );
  for (const fixture of fixtures) {
    const digestInput = fixture.files.map(file => `${file.sha256}  ${file.path}\n`).join("");
    assertBaseline(sha256(digestInput) === fixture.treeSha256, `${fixture.name} tree hash is inconsistent`);
    assertBaseline(
      fixture.files.reduce((total, file) => total + file.bytes, 0) === fixture.bytes,
      `${fixture.name} byte count is inconsistent`
    );
  }

  const inputFiles = report.inputs?.files ?? [];
  assertBaseline(new Set(inputFiles.map(file => file.path)).size === inputFiles.length, "input paths are not unique");
  assertBaseline(
    inputFiles.every(file => Number.isInteger(file.bytes) && file.bytes >= 0 && /^[0-9a-f]{64}$/.test(file.sha256)),
    "input file identity is malformed"
  );
  const inputDigest = sha256(inputFiles.map(file => `${file.sha256}  ${file.path}\n`).join(""));
  assertBaseline(inputDigest === report.inputs.sha256, "input manifest hash is inconsistent");

  const inputByPath = new Map(inputFiles.map(file => [file.path, file]));
  for (const fixture of fixtures) {
    for (const file of fixture.files) {
      assertBaseline(sameJson(inputByPath.get(file.path), file), `${file.path} disagrees with the input manifest`);
    }
  }

  const contracts = report.legacyContracts?.contracts ?? [];
  assertBaseline(
    sameJson(contracts.map(contract => contract.path), CONTRACTS),
    "legacy contract set or order drifted"
  );
  for (const field of [
    "prettyBytes",
    "minifiedBytes",
    "minifiedExpandedBytes",
    "inlineEvidenceDeltaBytes",
    "inlineEvidenceNodeBytes"
  ]) {
    assertBaseline(
      contracts.reduce((total, contract) => total + contract[field], 0) ===
        report.legacyContracts.aggregate[field],
      `legacy contract aggregate ${field} is inconsistent`
    );
  }
  assertBaseline(report.rc3Audit?.integrityVerified === true, "RC.3 package integrity was not verified");
  assertBaseline(
    report.rc3Audit?.allConcreteExportTargetsExist === true,
    "an RC.3 concrete export target is missing"
  );
}

export function buildBaseline() {
  throw new Error(
    "Phase 0 is immutable historical evidence; current producers cannot recapture or decode it"
  );
}

function gib(kib) {
  return round(kib / 1024 / 1024, 3);
}

export function renderMarkdown(report) {
  const classifications = report.ecosystem.classifications.summary;
  const lines = [
    "# Phase 0 package-contract baseline",
    "",
    `Captured at \`${report.capturedAt}\` from \`${report.repository.head}\` on \`${report.repository.branch}\`.`,
    "",
    "This is the reproducible comparison authority for the legacy package-contract implementation. Published Solid 2 RC.3 package bytes are authoritative for future behavior, while the currently bundled Solid 2 contracts remain RC.0 inputs; this report does not certify RC.3 semantics.",
    "",
    "## Exit result",
    "",
    `- ${report.ecosystem.verification.rows} verifier rows were classified exactly once.`,
    `- ${classifications.byOutcome.verified} verified; ${classifications.byOutcome.refused} refused; ${classifications.byOutcome["generate-failure"]} generation failures; ${classifications.byOutcome["no-runtime"]} without a resolvable Solid runtime.`,
    `- Input manifest: \`${report.inputs.sha256}\` over ${report.inputs.files.length} files.`,
    `- ${report.fixtureFreeze.fixtureCount} representative legacy fixtures are hash-frozen.`,
    "- All three measured commands exited successfully with `SOLID_CHECKER_GATE_CACHE=0`.",
    "",
    "## Exact pins",
    "",
    "| Input | Identity |",
    "| --- | --- |",
    `| Solid 2 compiler | \`${report.pins.solid2Compiler}\` (semantic trace ${report.pins.solid2SemanticTraceVersion}) |`,
    `| Solid 1 compiler | \`${report.pins.solid1Compiler}\` |`,
    `| Type Facts | \`${report.pins.typeFacts}\` |`,
    `| Checker binary | \`${report.pins.checkerBinaries.native.sha256}\` |`,
    `| Type Facts binary | \`${report.pins.checkerBinaries.typeFacts.sha256}\` |`,
    `| Published Solid authority | RC.3 at \`${report.pins.publishedSolidAuthority.gitHead}\` |`,
    "",
    "## Classification ownership",
    "",
    "| Owner | Rows |",
    "| --- | ---: |",
    ...Object.entries(classifications.byOwner).map(([owner, count]) => `| ${owner} | ${count} |`),
    "",
    "The machine report contains every row's exact probe ID, outcome, primary and secondary owner, failure class, stable reason, and verifier blocker. Zero rows are currently assigned to compiler-facts or TypeScript ownership; those zeroes are explicit rather than omitted.",
    "",
    "## Legacy schema and contract size",
    "",
    `The schema is ${report.legacySchema.prettyBytes.toLocaleString()} pretty bytes and ${report.legacySchema.minifiedBytes.toLocaleString()} minified bytes, with maximum measured object depth ${report.legacySchema.maximumObjectDepth}.`,
    "",
    "| Contract | Pretty | Minified | Expanded | Evidence delta |",
    "| --- | ---: | ---: | ---: | ---: |",
    ...report.legacyContracts.contracts.map(value =>
      `| ${value.package} | ${value.prettyBytes.toLocaleString()} | ${value.minifiedBytes.toLocaleString()} | ${value.minifiedExpandedBytes.toLocaleString()} | ${value.inlineEvidenceDeltaBytes.toLocaleString()} |`
    ),
    "",
    `Across the bundle, minified main documents total ${report.legacyContracts.aggregate.minifiedBytes.toLocaleString()} bytes, expanded documents total ${report.legacyContracts.aggregate.minifiedExpandedBytes.toLocaleString()} bytes, and inline evidence accounts for a ${report.legacyContracts.aggregate.inlineEvidenceDeltaBytes.toLocaleString()}-byte serialized delta. Legacy sidecar evidence is not applicable.`,
    "",
    "## Time and memory",
    "",
    "| Measurement | Wall time | Peak process-tree RSS |",
    "| --- | ---: | ---: |",
    `| Ecosystem generation | ${(report.measurements.ecosystemGeneration.elapsedMs / 1000).toFixed(1)} s | ${gib(report.measurements.ecosystemGeneration.memory.maxProcessTreeRssKiB)} GiB |`,
    `| Ecosystem verification | ${(report.measurements.ecosystemVerification.elapsedMs / 1000).toFixed(1)} s | ${gib(report.measurements.ecosystemVerification.memory.maxProcessTreeRssKiB)} GiB |`,
    `| Legacy contract corpus | ${(report.measurements.legacyContractCorpus.elapsedMs / 1000).toFixed(2)} s | ${gib(report.measurements.legacyContractCorpus.memory.maxProcessTreeRssKiB)} GiB |`,
    "",
    `Legacy JavaScript parse+expand of all ${report.legacyContracts.performance.load.contractsPerIteration} bundles: p50 ${report.legacyContracts.performance.load.p50Ms} ms, p95 ${report.legacyContracts.performance.load.p95Ms} ms. Direct normalized lookup: p50 ${report.legacyContracts.performance.query.p50NsPerLookup} ns, p95 ${report.legacyContracts.performance.query.p95NsPerLookup} ns. The Rust consumer has no isolated query seam, so Rust cost is represented by the end-to-end corpus measurement.`,
    "",
    "## Frozen fixtures",
    "",
    "| Fixture | Purpose | Tree SHA-256 |",
    "| --- | --- | --- |",
    ...report.fixtureFreeze.fixtures.map(value => `| ${value.name} | ${value.purpose} | \`${value.treeSha256}\` |`),
    "",
    "## Known boundaries",
    "",
    "- The RC.3 audit proves exact tarball integrity, manifests, package contents, and concrete export-target existence. It does not retain a complete transitive installed dependency/declaration closure; Phase 7 owns that proof.",
    "- The fresh ecosystem generation run observed one 600-second package-install timeout. Independent verification retried the row successfully and classified it as a refusal; both observations are preserved.",
    "- Verification success means the current legacy proof policy accepted the document. It is a comparison baseline, not evidence that the replacement proof model has already been implemented.",
    "- Current bundled Solid 2 contracts are RC.0 structural inputs and must never be cited as RC.3 semantic authority.",
    "",
    "## Reproduction",
    "",
    "The raw command arrays, timestamps, exit status, sampling method, sample count, and peak RSS are preserved under `benchmarks/package-contract-v2/phase0/measurements/`. Re-run generation and verification with stable checker binaries and `SOLID_CHECKER_GATE_CACHE=0`, then run:",
    "",
    "```sh",
    "bun scripts/package-contract-v2-phase0.mjs --capture-current --output-json /tmp/phase0-current.json --output-markdown /tmp/phase0-current.md",
    "bun scripts/package-contract-v2-phase0.mjs --check",
    "```",
    ""
  ];
  return lines.join("\n");
}

function parseArgs(argv) {
  const options = {
    outputJson: "benchmarks/package-contract-v2/phase0/baseline.json",
    outputMarkdown: "benchmarks/package-contract-v2/phase0/baseline.md",
    check: false,
    outputJsonExplicit: false,
    outputMarkdownExplicit: false
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--check") options.check = true;
    else if (argument === "--capture-current") {
      throw new Error("Phase 0 is immutable historical evidence; --capture-current was retired in Phase 14");
    }
    else if (argument === "--output-json") {
      options.outputJson = argv[++index];
      options.outputJsonExplicit = true;
    } else if (argument === "--output-markdown") {
      options.outputMarkdown = argv[++index];
      options.outputMarkdownExplicit = true;
    } else throw new Error(`unknown argument ${argument}`);
  }
  if (!options.check) throw new Error("Phase 0 supports only --check after producer migration");
  return options;
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.check) {
    const jsonBytes = read(options.outputJson);
    const markdownBytes = read(options.outputMarkdown);
    if (sha256(jsonBytes) !== FROZEN_BASELINE_JSON_SHA256) {
      throw new Error(`${options.outputJson} is not the frozen Phase 0 machine report`);
    }
    if (sha256(markdownBytes) !== FROZEN_BASELINE_MARKDOWN_SHA256) {
      throw new Error(`${options.outputMarkdown} is not the frozen Phase 0 human report`);
    }
    const report = JSON.parse(jsonBytes);
    assertFrozenBaseline(report);
    if (markdownBytes.toString() !== renderMarkdown(report)) {
      throw new Error(`${options.outputMarkdown} is not the rendering of ${options.outputJson}`);
    }
    console.log(`phase0 baseline: ${report.ecosystem.classifications.rows.length} rows and ${report.fixtureFreeze.fixtureCount} fixtures verified as frozen evidence`);
    return;
  }
}

if (import.meta.main) main();
