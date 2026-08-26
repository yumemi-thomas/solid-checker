#!/usr/bin/env bun

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { performance } from "node:perf_hooks";

import { expandContract } from "../packages/cli/scripts/contract-document.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

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

const INPUTS = [
  "schema/solid-reactivity.schema.json",
  "rust/Cargo.toml",
  "rust/dialects/solid-v2/compiler/src/lib.rs",
  "bin/solid-typefacts.buildinfo",
  "scripts/ecosystem-benchmark/manifest.json",
  "benchmarks/ecosystem/report.json",
  "benchmarks/ecosystem/verification-report.json",
  "benchmarks/package-contract-v2/phase0/rc3/audit.json",
  "benchmarks/package-contract-v2/phase0/measurements/ecosystem-generation.json",
  "benchmarks/package-contract-v2/phase0/measurements/ecosystem-verification.json",
  "benchmarks/package-contract-v2/phase0/measurements/contract-corpus.json",
  ...CONTRACTS
];

function bytes(value) {
  return Buffer.byteLength(value);
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function read(relativePath) {
  return readFileSync(resolve(root, relativePath));
}

function json(relativePath) {
  return JSON.parse(read(relativePath));
}

function percentile(values, fraction) {
  if (!values.length) return 0;
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * fraction) - 1)];
}

function round(value, digits = 3) {
  const scale = 10 ** digits;
  return Math.round(value * scale) / scale;
}

export function schemaMetrics(schema) {
  const metrics = {
    definitions: Object.keys(schema.$defs ?? {}).length,
    namedProperties: 0,
    requiredNames: 0,
    refs: 0,
    oneOf: 0,
    anyOf: 0,
    allOf: 0,
    enumDeclarations: 0,
    enumValues: 0,
    maximumObjectDepth: 0
  };
  const visit = (value, depth) => {
    if (!value || typeof value !== "object") return;
    metrics.maximumObjectDepth = Math.max(metrics.maximumObjectDepth, depth);
    if (Array.isArray(value)) {
      for (const child of value) visit(child, depth + 1);
      return;
    }
    metrics.namedProperties += Object.keys(value.properties ?? {}).length;
    metrics.requiredNames += value.required?.length ?? 0;
    metrics.refs += "$ref" in value ? 1 : 0;
    metrics.oneOf += value.oneOf?.length ? 1 : 0;
    metrics.anyOf += value.anyOf?.length ? 1 : 0;
    metrics.allOf += value.allOf?.length ? 1 : 0;
    if (value.enum) {
      metrics.enumDeclarations += 1;
      metrics.enumValues += value.enum.length;
    }
    for (const child of Object.values(value)) visit(child, depth + 1);
  };
  visit(schema, 1);
  return metrics;
}

function stripEvidence(value) {
  if (Array.isArray(value)) return value.map(stripEvidence);
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value)
      .filter(([key]) => key !== "evidence")
      .map(([key, child]) => [key, stripEvidence(child)])
  );
}

function evidenceNodeBytes(value) {
  if (Array.isArray(value)) return value.reduce((total, child) => total + evidenceNodeBytes(child), 0);
  if (!value || typeof value !== "object") return 0;
  return Object.entries(value).reduce(
    (total, [key, child]) =>
      total + (key === "evidence" ? bytes(JSON.stringify(child)) : evidenceNodeBytes(child)),
    0
  );
}

export function measureContract(relativePath) {
  const raw = read(relativePath);
  const document = JSON.parse(raw);
  const minified = JSON.stringify(document);
  const withoutEvidence = JSON.stringify(stripEvidence(document));
  const expanded = expandContract(document);
  return {
    path: relativePath,
    package: `${document.package.name}@${document.package.version}`,
    prettyBytes: raw.byteLength,
    minifiedBytes: bytes(minified),
    minifiedExpandedBytes: bytes(JSON.stringify(expanded)),
    minifiedWithoutEvidenceBytes: bytes(withoutEvidence),
    inlineEvidenceDeltaBytes: bytes(minified) - bytes(withoutEvidence),
    inlineEvidenceNodeBytes: evidenceNodeBytes(document),
    summaries: Object.keys(document.summaries).length,
    entrypoints: Object.keys(document.entrypoints).length,
    expandedExports: Object.values(expanded.entrypoints).reduce(
      (total, entrypoint) => total + Object.keys(entrypoint.exports).length,
      0
    )
  };
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

function benchmarkContracts(loadIterations, queryIterations) {
  const raw = CONTRACTS.map(path => read(path));
  const loadSamples = [];
  for (let index = 0; index < loadIterations; index += 1) {
    const started = performance.now();
    for (const value of raw) expandContract(JSON.parse(value));
    loadSamples.push(performance.now() - started);
  }
  const expanded = raw.map(value => expandContract(JSON.parse(value)));
  const lookups = expanded.flatMap(contract =>
    Object.values(contract.entrypoints).flatMap(entrypoint =>
      Object.keys(entrypoint.exports).map(name => [entrypoint.exports, name])
    )
  );
  let sink = 0;
  const batchSize = Math.max(1000, Math.ceil(queryIterations / 20));
  const querySamples = [];
  let completed = 0;
  while (completed < queryIterations) {
    const count = Math.min(batchSize, queryIterations - completed);
    const started = performance.now();
    for (let offset = 0; offset < count; offset += 1) {
      const [exports, name] = lookups[(completed + offset) % lookups.length];
      if (exports[name]) sink += 1;
    }
    querySamples.push(((performance.now() - started) * 1_000_000) / count);
    completed += count;
  }
  if (sink !== queryIterations) throw new Error("contract query benchmark lost an export lookup");
  return {
    method: "JavaScript legacy decoder path; parse plus expand all bundled contracts, then direct normalized export lookup",
    limitation: "The current Rust consumer has no isolated contract-query benchmark seam; end-to-end Rust cost is represented by the uncached corpus measurements.",
    load: {
      contractsPerIteration: CONTRACTS.length,
      iterations: loadIterations,
      p50Ms: round(percentile(loadSamples, 0.5)),
      p95Ms: round(percentile(loadSamples, 0.95)),
      maxMs: round(Math.max(...loadSamples))
    },
    query: {
      availableExports: lookups.length,
      operations: queryIterations,
      p50NsPerLookup: round(percentile(querySamples, 0.5)),
      p95NsPerLookup: round(percentile(querySamples, 0.95)),
      maxNsPerLookup: round(Math.max(...querySamples))
    }
  };
}

function git(...args) {
  return execFileSync("git", args, { cwd: root, encoding: "utf8" }).trim();
}

function pinFromCargo(name, cargo) {
  const expression = new RegExp(`${name.replace(/[.*+?^${}()|[\\]\\]/g, "\\$&")}\\s*=\\s*\\{[^\\n]*rev\\s*=\\s*"([0-9a-f]+)"`);
  const match = cargo.match(expression);
  if (!match) throw new Error(`could not read ${name} pin from rust/Cargo.toml`);
  return match[1];
}

function inputManifest(fixtures) {
  const files = INPUTS.map(path => ({ path, bytes: read(path).byteLength, sha256: sha256(read(path)) }));
  for (const fixture of fixtures) files.push(...fixture.files);
  const deduplicated = [...new Map(files.map(file => [file.path, file])).values()].sort((a, b) =>
    a.path.localeCompare(b.path)
  );
  return {
    sha256: sha256(deduplicated.map(file => `${file.sha256}  ${file.path}\n`).join("")),
    files: deduplicated
  };
}

function binaryRecord(record) {
  const path = record.path;
  let current = null;
  try {
    const value = readFileSync(path);
    current = { sha256: sha256(value), bytes: value.byteLength };
  } catch {
    // The durable evidence is the hash copied into the verifier report. The
    // scratch binary is intentionally not a repository artifact.
  }
  return {
    recordedPath: path,
    sha256: record.sha256,
    bytes: record.size,
    presentAtReportGeneration: Boolean(current),
    hashMatchedAtReportGeneration: current ? current.sha256 === record.sha256 : null
  };
}

export function assertSuccessfulCacheDisabledMeasurements(measurements) {
  for (const [name, measurement] of Object.entries(measurements)) {
    if (measurement.exitCode !== 0 || !measurement.command.includes("SOLID_CHECKER_GATE_CACHE=0")) {
      throw new Error(`${name} is not a successful cache-disabled measurement`);
    }
  }
}

export function buildBaseline({ loadIterations = 300, queryIterations = 200_000 } = {}) {
  const generation = json("benchmarks/ecosystem/report.json");
  const verification = json("benchmarks/ecosystem/verification-report.json");
  const rc3 = json("benchmarks/package-contract-v2/phase0/rc3/audit.json");
  const manifest = json("scripts/ecosystem-benchmark/manifest.json");
  const measurements = {
    ecosystemGeneration: json("benchmarks/package-contract-v2/phase0/measurements/ecosystem-generation.json"),
    ecosystemVerification: json("benchmarks/package-contract-v2/phase0/measurements/ecosystem-verification.json"),
    legacyContractCorpus: json("benchmarks/package-contract-v2/phase0/measurements/contract-corpus.json")
  };
  assertSuccessfulCacheDisabledMeasurements(measurements);
  const rows = classifyVerificationRows(verification, generation);
  const fixtures = Object.entries(FROZEN_FIXTURES).map(([name, purpose]) =>
    freezeFixture(name, purpose)
  );
  const cargo = read("rust/Cargo.toml").toString();
  const traceSource = read("rust/dialects/solid-v2/compiler/src/lib.rs").toString();
  const traceVersion = Number(traceSource.match(/READS_TRACE_VERSION:\s*u32\s*=\s*(\d+)/)?.[1]);
  if (!Number.isInteger(traceVersion)) throw new Error("could not read Solid 2 trace version");
  const schemaRaw = read("schema/solid-reactivity.schema.json");
  const schema = JSON.parse(schemaRaw);
  const contracts = CONTRACTS.map(measureContract);
  const checker = verification.checker;
  const report = {
    schemaVersion: 1,
    documentKind: "solid-checker-package-contract-phase0-baseline",
    capturedAt: new Date().toISOString(),
    scope: {
      phase: 0,
      authority: "legacy package-contract comparison baseline",
      caution: "Published RC.3 artifacts are package authority; current bundled Solid 2 contracts and checker dialect authority remain RC.0 until later migration phases."
    },
    repository: {
      head: git("rev-parse", "HEAD"),
      branch: git("branch", "--show-current"),
      status: git("status", "--short").split("\n").filter(Boolean)
    },
    pins: {
      solid2Compiler: pinFromCargo("dom-expressions-compiler", cargo),
      solid1Compiler: pinFromCargo("solid1-dom-expressions-compiler", cargo),
      typeFacts: pinFromCargo("typefacts", cargo),
      typeFactsBuildInfo: read("bin/solid-typefacts.buildinfo").toString().trim(),
      solid2SemanticTraceVersion: traceVersion,
      legacyContractSchemaVersion: schema.properties.schemaVersion.const,
      legacyCompilerFactsProtocol: schema.properties.compilerFactsProtocol.const,
      publishedSolidAuthority: {
        version: manifest.auditedSolid2,
        gitHead: rc3.gitHead,
        packages: rc3.packages.map(value => ({
          name: value.name,
          version: value.version,
          integrity: value.registry.integrity,
          tarballSha256: value.tarball.sha256,
          manifestSha256: value.manifest.sha256,
          exportMapSha256: value.manifest.exportMapSha256,
          fileManifestSha256: value.files.manifestSha256
        }))
      },
      checkerBinaries: {
        native: binaryRecord(checker.nativeBin),
        typeFacts: binaryRecord(checker.typeFactsBin)
      }
    },
    rc3Audit: {
      path: "benchmarks/package-contract-v2/phase0/rc3/audit.json",
      sha256: sha256(read("benchmarks/package-contract-v2/phase0/rc3/audit.json")),
      integrityVerified: rc3.packages.every(value => value.integrity.verified),
      allConcreteExportTargetsExist: rc3.packages.every(value =>
        value.exportTargets.filter(target => !target.pattern).every(target => target.exists)
      ),
      exactTransitiveClosureRetained: false,
      closureLimitation: "The RC.3 audit preserves exact package contents and export targets but not an isolated exact transitive dependency/declaration installation closure; Phase 7 owns that proof."
    },
    legacySchema: {
      path: "schema/solid-reactivity.schema.json",
      prettyBytes: schemaRaw.byteLength,
      minifiedBytes: bytes(JSON.stringify(schema)),
      ...schemaMetrics(schema)
    },
    legacyContracts: {
      contracts,
      aggregate: {
        prettyBytes: contracts.reduce((total, value) => total + value.prettyBytes, 0),
        minifiedBytes: contracts.reduce((total, value) => total + value.minifiedBytes, 0),
        minifiedExpandedBytes: contracts.reduce((total, value) => total + value.minifiedExpandedBytes, 0),
        inlineEvidenceDeltaBytes: contracts.reduce((total, value) => total + value.inlineEvidenceDeltaBytes, 0),
        inlineEvidenceNodeBytes: contracts.reduce((total, value) => total + value.inlineEvidenceNodeBytes, 0),
        sidecarEvidenceBytes: 0,
        sidecarEvidenceStatus: "not applicable: legacy contracts store evidence inline"
      },
      performance: benchmarkContracts(loadIterations, queryIterations)
    },
    ecosystem: {
      generation: {
        rows: generation.results.length,
        outcomes: generation.results.reduce((output, row) => {
          output[row.outcome] = (output[row.outcome] ?? 0) + 1;
          return output;
        }, {}),
        anomalies: classifyGenerationAnomalies(generation)
      },
      verification: verification.overall,
      classifications: {
        rows,
        summary: summarizeClassifications(rows),
        invariant: "Every verifier-selected row appears exactly once; unknown owner/failure classes abort report generation."
      }
    },
    measurements,
    fixtureFreeze: {
      fixtureCount: fixtures.length,
      fixtures,
      invariant: "Each digest covers every regular file currently present below the fixture directory, including ignored node_modules inputs."
    }
  };
  report.inputs = inputManifest(fixtures);
  return report;
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
    "bun scripts/package-contract-v2-phase0.mjs",
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
    check: false
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--check") options.check = true;
    else if (argument === "--output-json") options.outputJson = argv[++index];
    else if (argument === "--output-markdown") options.outputMarkdown = argv[++index];
    else throw new Error(`unknown argument ${argument}`);
  }
  return options;
}

function comparable(report) {
  const copy = structuredClone(report);
  delete copy.capturedAt;
  delete copy.repository;
  delete copy.legacyContracts.performance;
  return copy;
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const report = buildBaseline();
  const markdown = renderMarkdown(report);
  const jsonText = `${JSON.stringify(report, null, 2)}\n`;
  if (options.check) {
    const current = JSON.parse(read(options.outputJson));
    if (JSON.stringify(comparable(current)) !== JSON.stringify(comparable(report))) {
      throw new Error(`${options.outputJson} does not match current Phase 0 inputs`);
    }
    const stableMarkdown = value => value
      .replace(/Captured at `[^`]+` from `[^`]+` on `[^`]+`\./, "Captured at `<dynamic>` from `<dynamic>` on `<dynamic>`.")
      .replace(/Legacy JavaScript parse\+expand[^\n]+/, "Legacy JavaScript performance: <dynamic>");
    if (stableMarkdown(read(options.outputMarkdown).toString()) !== stableMarkdown(markdown)) {
      throw new Error(`${options.outputMarkdown} does not match current Phase 0 inputs`);
    }
    console.log(`phase0 baseline: ${report.ecosystem.classifications.rows.length} rows and ${report.fixtureFreeze.fixtureCount} fixtures verified`);
    return;
  }
  writeFileSync(resolve(root, options.outputJson), jsonText);
  writeFileSync(resolve(root, options.outputMarkdown), markdown);
  console.log(`phase0 baseline: wrote ${options.outputJson} and ${options.outputMarkdown}`);
}

if (import.meta.main) main();
