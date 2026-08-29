import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { arch, platform } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { runProbeSessions } from "../packages/cli/scripts/contract-probe-driver.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const PHASE16 = join(ROOT, "benchmarks/package-contract-v2/phase16");
const REPORT_PATH = join(PHASE16, "report.json");
const REPORT_MARKDOWN_PATH = join(PHASE16, "report.md");
const REFUSALS_PATH = join(PHASE16, "refusals.json");

const INPUTS = Object.freeze({
  ecosystem: "benchmarks/ecosystem/report.json",
  historicalVerification: "benchmarks/ecosystem/verification-report.json",
  manifest: "scripts/ecosystem-benchmark/manifest.json",
  syntheticCorpus: "fixtures/package-contracts/corpus.json",
  phase13: "benchmarks/package-contract-v2/phase13/conformance.json",
  currentProbeRecipe: "scripts/package-contract-v2-phase16-probe-recipe.mjs"
});

const EXPECTED_FAMILIES = [
  "corvu",
  "kobalte",
  "motion-solidjs",
  "official-solid",
  "solid-devtools",
  "solid-primitives",
  "solid-recharts",
  "tanstack"
];

const REQUIRED_RC3 = [
  ["solid-js", "2.0.0-rc.3"],
  ["@solidjs/signals", "2.0.0-rc.3"],
  ["@solidjs/web", "2.0.0-rc.3"]
];

const DOMAIN_OWNERS = Object.freeze({
  callbacks: "call-target and callback-execution proof",
  reads: "reactive-read reachability proof",
  writes: "reactive-write reachability proof",
  creates: "resource-creation and ownership proof",
  invalidates: "invalidation reachability proof",
  throws: "error-edge and throw census",
  returns: "recursive return-value proof",
  cleanups: "cleanup-edge and lifetime proof",
  disposals: "resource-disposal and lifetime proof",
  recursiveValue: "exact recursive value-leaf proof"
});

function read(relative) {
  return readFileSync(join(ROOT, relative));
}

function json(relative) {
  return JSON.parse(read(relative).toString("utf8"));
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function percentage(part, whole) {
  return whole === 0 ? 0 : Math.round((part / whole) * 10000) / 100;
}

function distribution(values) {
  const sorted = values.filter(Number.isFinite).slice().sort((left, right) => left - right);
  if (sorted.length === 0) return { count: 0, p50: null, p95: null, max: null };
  const at = fraction => sorted[Math.max(0, Math.ceil(sorted.length * fraction) - 1)];
  return { count: sorted.length, p50: at(0.5), p95: at(0.95), max: sorted.at(-1) };
}

async function benchmarkCurrentProbeExecution(iterations = 25) {
  const recipePath = join(ROOT, INPUTS.currentProbeRecipe);
  const construction = `sha256:${sha256(readFileSync(recipePath))}`;
  const runtimeBuild = `sha256:${sha256(Buffer.from(`${process.execPath}\0${process.version}`))}`;
  const environment = {
    runtime: {
      name: "node",
      version: process.version,
      build: runtimeBuild,
      protocol: "solid-checker-runtime-probe-v2"
    },
    os: platform(),
    architecture: arch(),
    conditions: ["phase16"],
    sandbox: { kind: "process", policy: `sha256:${"7".repeat(64)}` }
  };
  const sessions = Array.from({ length: 2 }, (_, repeat) => ({
    id: `sha256:${String(repeat + 1).repeat(64)}`,
    module: basename(recipePath),
    construction,
    mode: { name: "phase16-current", artifactCase: "phase16", environment },
    repeat,
    drain: [
      { kind: "microtasks", maxTurns: 2 },
      { kind: "macrotasks", maxTurns: 1 }
    ],
    policy: { timeoutMillis: 5_000 }
  }));
  const plan = {
    format: "solid-checker-runtime-probe-plan",
    schemaVersion: 2,
    sessions
  };
  const samples = [];
  for (let iteration = 0; iteration < iterations; iteration += 1) {
    const started = process.hrtime.bigint();
    const runs = await runProbeSessions(
      plan,
      join(dirname(recipePath), "package-contract-v2-phase16-probe-request.json")
    );
    const elapsed = Number(process.hrtime.bigint() - started) / 1_000_000 / sessions.length;
    assert.equal(runs.length, sessions.length);
    assert.ok(runs.every(run => run.outcome?.kind === "completed"));
    assert.ok(
      runs.every(run =>
        run.outcome.events.some(event => event.marker === "phase16-current-probe")
      )
    );
    samples.push(Math.round(elapsed * 100) / 100);
  }
  return {
    authority: "current stable-v1 fresh-process probe driver with a deterministic witness recipe",
    semanticAcceptance: false,
    iterations,
    sessionsPerIteration: sessions.length,
    millisecondsPerIsolatedSession: distribution(samples)
  };
}

function outcomeSummary(results) {
  const complete = results.filter(result => result.outcome === "success").length;
  const partial = results.filter(result => result.outcome === "partial-success").length;
  return {
    rows: results.length,
    complete,
    partial,
    refused: results.length - complete - partial,
    completePercentage: percentage(complete, results.length),
    generatablePercentage: percentage(complete + partial, results.length)
  };
}

function refusalOwner(className) {
  if (/timeout|install|integrity/.test(className)) return "corpus infrastructure";
  if (/entrypoint|esm|cjs|export|artifact/.test(className)) return "artifact resolver and generator";
  if (/dependency|contract/.test(className)) return "accepted dependency composition";
  if (/compiler/.test(className)) return "compiler semantic facts";
  if (/type|signature|callback|reactive/.test(className)) return "Type Facts and semantic proof";
  return "package-contract generator";
}

export function buildRefusalReport(ecosystem, phase13) {
  const failures = ecosystem.results
    .filter(result => result.outcome === "failure")
    .map(result => ({
      probeId: result.probeId,
      package: result.package,
      family: result.family,
      class: result.class,
      owner: refusalOwner(result.class),
      reason: result.signature
    }))
    .sort((left, right) => left.probeId.localeCompare(right.probeId));
  const partials = ecosystem.results
    .filter(result => result.outcome === "partial-success")
    .map(result => ({
      probeId: result.probeId,
      package: result.package,
      family: result.family,
      refusedEntrypoints: result.refusedEntrypoints,
      refusedArtifactCases: result.refusedArtifactCases,
      artifactCaseRefusals: result.contractContent?.artifactCaseRefusals ?? [],
      owner: "artifact resolver and generator",
      reason: "one or more exact entrypoints or artifact cases were refused while independent cases remained generatable"
    }))
    .sort((left, right) => left.probeId.localeCompare(right.probeId));
  const unknowns = Object.entries(ecosystem.combined.contractContent.unknownByDomain)
    .map(([domain, count]) => ({
      domain,
      count,
      owner: DOMAIN_OWNERS[domain] ?? "named conformance authority",
      reason:
        domain === "recursiveValue"
          ? "at least one exact recursive value leaf remains unknown"
          : "the proposal does not carry accepted exhaustive proof for this local claim domain"
    }))
    .filter(row => row.count > 0);
  const conformance = phase13.rows
    .flatMap(row => row.normalized.openDomains.map(domain => ({ domain, row: row.id })))
    .sort((left, right) =>
      `${left.domain}:${left.row}`.localeCompare(`${right.domain}:${right.row}`)
    );
  return {
    schemaVersion: 1,
    documentKind: "solid-checker-phase16-refusal-report",
    missingEvidenceIsNegativeProof: false,
    generatedProposalFailures: failures,
    partialCaseRefusals: partials,
    openClaimDomains: unknowns,
    solid2ConformanceOpenDomains: conformance
  };
}

function assertOrdinaryAnalysisBoundary(source = readFileSync(join(
  ROOT,
  "rust/crates/solid-reactive-ir/src/contract_semantics/consumer.rs"
), "utf8")) {
  for (const forbidden of ["std::fs", "std::process", "reqwest", "ureq", "TcpStream", "Command::new"])
    assert.ok(!source.includes(forbidden), `ordinary contract consumer contains ${forbidden}`);
  const index = source.match(/pub struct AcceptedContractIndex \{[\s\S]*?\n\}/)?.[0];
  assert.ok(index, "AcceptedContractIndex definition is missing");
  assert.ok(!/sidecar|document_bytes|receipt_bytes|package_root/i.test(index));
  assert.match(source, /pub fn resolve<'a>\(/);
}

export function buildPhase16Report({ ecosystem, historicalVerification, manifest, syntheticCorpus, phase13, accepted, probeExecution }) {
  assertOrdinaryAnalysisBoundary();
  assert.equal(ecosystem.scope.kind, "full");
  assert.equal(ecosystem.scope.probesRun, ecosystem.results.length);
  const official = ecosystem.results.filter(result => result.status !== "supplemental");
  const primitives = official.filter(result => result.family === "solid-primitives");
  const overall = outcomeSummary(official);
  const solidPrimitives = outcomeSummary(primitives);
  assert.ok(overall.generatablePercentage >= 85, `generatable coverage fell to ${overall.generatablePercentage}%`);
  assert.ok(solidPrimitives.generatablePercentage >= 90, `Solid Primitives fell to ${solidPrimitives.generatablePercentage}%`);

  const families = [...new Set(manifest.rows.map(row => row.family))].sort();
  assert.deepEqual(families, EXPECTED_FAMILIES);
  for (const [name, version] of REQUIRED_RC3) {
    const row = ecosystem.results.find(
      result =>
        result.package === name &&
        result.version === version &&
        ["success", "partial-success"].includes(result.outcome)
    );
    assert.ok(row, `${name}@${version} is not a generatable official corpus row`);
  }
  assert.equal(syntheticCorpus.fixtures.length, 39);
  assert.equal(phase13.rows.length, 16);
  assert.equal(accepted.corpus.receiptIssuedArtifactCases, 24);
  assert.equal(accepted.compactness.rawEvidenceRetainedByOrdinaryAnalysis, 0);
  assert.deepEqual(accepted.ordinaryAnalysis, {
    input: "AcceptedContractIndex / receipt-validated normalized semantics",
    rawSidecarBytes: 0,
    packageCodeExecution: false,
    networkAccess: false,
    queryFileReads: false
  });

  const main = accepted.compactness.canonicalMainBytes;
  assert.ok(main.p50 <= 8 * 1024, `artifact-case p50 ${main.p50} exceeds 8 KiB`);
  assert.ok(main.p95 <= 32 * 1024, `artifact-case p95 ${main.p95} exceeds 32 KiB`);
  assert.ok(main.p50 <= 16 * 1024, `package p50 ${main.p50} exceeds 16 KiB`);
  assert.ok(main.p95 <= 128 * 1024, `package p95 ${main.p95} exceeds 128 KiB`);
  assert.ok(main.max <= 1024 * 1024, `main document max ${main.max} exceeds 1 MiB`);
  assert.equal(accepted.compactness.proofEvidenceBytes.count, 24);
  assert.equal(accepted.compactness.acceptanceReceiptBytes.count, 24);
  const ecosystemProposalWire = ecosystem.combined.contractContent.wireBytes;
  assert.equal(
    ecosystemProposalWire.canonicalMain.count,
    overall.complete + overall.partial,
    "every generatable ecosystem proposal must contribute a wire-size sample"
  );
  assert.equal(ecosystem.combined.contractContent.probesFullyProven, 0);
  assert.ok(accepted.performance.acceptedCorpusLoadNs.p95 > 0);
  assert.ok(accepted.performance.normalizedQueryNsPerExport.p95 > 0);
  assert.ok(accepted.performance.memory.postLoadPeakResidentKiB > 0);
  assert.equal(probeExecution.semanticAcceptance, false);
  assert.equal(probeExecution.millisecondsPerIsolatedSession.count, probeExecution.iterations);
  assert.ok(probeExecution.millisecondsPerIsolatedSession.p95 > 0);

  const refusalReport = buildRefusalReport(ecosystem, phase13);
  assert.ok(refusalReport.generatedProposalFailures.every(row => row.class && row.owner && row.reason));
  assert.ok(refusalReport.openClaimDomains.every(row => row.domain && row.owner && row.reason));

  const generationMs = distribution(
    official.map(result => result.generationDurationMs).filter(Number.isFinite)
  );
  return {
    schemaVersion: 1,
    documentKind: "solid-checker-package-contract-phase16-report",
    generatedAt: ecosystem.finishedAt,
    authority: {
      solid: "2.0.0-rc.3",
      proofRule: "coverage never closes a claim; only checked proof and a receipt authorize analysis",
      inputs: Object.fromEntries(
        Object.entries(INPUTS).map(([name, path]) => [name, { path, sha256: sha256(read(path)) }])
      )
    },
    corpus: {
      officialRc3Packages: REQUIRED_RC3.map(([name, version]) => `${name}@${version}`),
      ecosystemFamilies: families,
      ecosystem: overall,
      solidPrimitives,
      syntheticGeneratorFixtures: syntheticCorpus.fixtures.length,
      solid2ConformanceRows: phase13.rows.length,
      preservedReceiptIssuedRows: accepted.corpus.receiptIssuedArtifactCases,
      refusalCounts: {
        generatedFailures: refusalReport.generatedProposalFailures.length,
        partialEntrypoints: refusalReport.partialCaseRefusals.reduce(
          (total, row) => total + (row.refusedEntrypoints ?? 0),
          0
        ),
        partialArtifactCases: refusalReport.partialCaseRefusals.reduce(
          (total, row) => total + (row.refusedArtifactCases ?? 0),
          0
        ),
        locallyOpenClaimDomains: refusalReport.openClaimDomains.length,
        conformanceOpenRows: refusalReport.solid2ConformanceOpenDomains.length
      }
    },
    compactness: {
      ...accepted.compactness,
      ecosystemProposalWireBytes: ecosystemProposalWire
    },
    performance: {
      ecosystemGenerationMs: generationMs,
      currentAcceptedCorpus: accepted.performance,
      currentRuntimeProbeProcessExecution: probeExecution,
      runtimeProbeExecutionReference: {
        authority: "Phase 0 exact 418-row isolated-process run; execution-envelope reference only",
        currentSemanticAcceptance: false,
        distribution: historicalVerification.phaseWallMs.probe
      }
    },
    ordinaryAnalysis: {
      ...accepted.ordinaryAnalysis,
      sourceGate: "solid-reactive-ir AcceptedContractIndex has no filesystem, process, network, document-byte, receipt-byte, or sidecar field"
    }
  };
}

export function assertPhase16Report(report, refusalReport, sources) {
  assert.equal(report.schemaVersion, 1);
  assert.equal(report.documentKind, "solid-checker-package-contract-phase16-report");
  for (const [name, path] of Object.entries(INPUTS)) {
    assert.equal(report.authority.inputs[name].path, path);
    assert.equal(report.authority.inputs[name].sha256, sha256(read(path)), `${name} input drifted`);
  }
  assert.deepEqual(refusalReport, buildRefusalReport(sources.ecosystem, sources.phase13));
  assert.deepEqual(
    report,
    buildPhase16Report({ ...sources, probeExecution: report.performance.currentRuntimeProbeProcessExecution, accepted: { corpus: {
      receiptIssuedArtifactCases: report.corpus.preservedReceiptIssuedRows
    }, compactness: report.compactness, performance: report.performance.currentAcceptedCorpus,
    ordinaryAnalysis: Object.fromEntries(Object.entries(report.ordinaryAnalysis).filter(([key]) => key !== "sourceGate")) } })
  );
}

function renderMarkdown(report) {
  const main = report.compactness.canonicalMainBytes;
  const evidence = report.compactness.proofEvidenceBytes;
  const ecosystemWire = report.compactness.ecosystemProposalWireBytes;
  const performance = report.performance.currentAcceptedCorpus;
  return `# Phase 16 corpus, compactness, and performance report

- Solid authority: \`${report.authority.solid}\`
- Ecosystem: ${report.corpus.ecosystem.complete}/${report.corpus.ecosystem.rows} complete, ${report.corpus.ecosystem.partial} partial (${report.corpus.ecosystem.generatablePercentage}% generatable)
- Solid Primitives: ${report.corpus.solidPrimitives.complete}/${report.corpus.solidPrimitives.rows} complete, ${report.corpus.solidPrimitives.partial} partial (${report.corpus.solidPrimitives.generatablePercentage}% generatable)
- Receipt-issued cases preserved: ${report.corpus.preservedReceiptIssuedRows}
- Synthetic generator fixtures: ${report.corpus.syntheticGeneratorFixtures}

## Compactness

| Measure | p50 | p95 | max |
| --- | ---: | ---: | ---: |
| Canonical main bytes | ${main.p50} | ${main.p95} | ${main.max} |
| Pretty main bytes | ${report.compactness.prettyMainBytes.p50} | ${report.compactness.prettyMainBytes.p95} | ${report.compactness.prettyMainBytes.max} |
| Normalized semantic debug bytes | ${report.compactness.normalizedSemanticDebugBytes.p50} | ${report.compactness.normalizedSemanticDebugBytes.p95} | ${report.compactness.normalizedSemanticDebugBytes.max} |
| Proof evidence bytes | ${evidence.p50} | ${evidence.p95} | ${evidence.max} |
| Acceptance receipt bytes | ${report.compactness.acceptanceReceiptBytes.p50} | ${report.compactness.acceptanceReceiptBytes.p95} | ${report.compactness.acceptanceReceiptBytes.max} |
| Ecosystem proposal main bytes | ${ecosystemWire.canonicalMain.p50} | ${ecosystemWire.canonicalMain.p95} | ${ecosystemWire.canonicalMain.max} |
| Ecosystem proposal-plan bytes | ${ecosystemWire.proposalPlan.p50} | ${ecosystemWire.proposalPlan.p95} | ${ecosystemWire.proposalPlan.max} |

Raw proof evidence retained by ordinary analysis: **${report.compactness.rawEvidenceRetainedByOrdinaryAnalysis} bytes**.

## Performance

| Phase | p50 | p95 | max | Unit |
| --- | ---: | ---: | ---: | --- |
| Ecosystem generation | ${report.performance.ecosystemGenerationMs.p50} | ${report.performance.ecosystemGenerationMs.p95} | ${report.performance.ecosystemGenerationMs.max} | ms / row |
| Current isolated runtime probe | ${report.performance.currentRuntimeProbeProcessExecution.millisecondsPerIsolatedSession.p50} | ${report.performance.currentRuntimeProbeProcessExecution.millisecondsPerIsolatedSession.p95} | ${report.performance.currentRuntimeProbeProcessExecution.millisecondsPerIsolatedSession.max} | ms / session |
| Proof-input generation | ${performance.proposalAndProofInputGenerationNs.p50} | ${performance.proposalAndProofInputGenerationNs.p95} | ${performance.proposalAndProofInputGenerationNs.max} | ns / accepted case |
| Verification and receipt | ${performance.proofVerificationAndReceiptNs.p50} | ${performance.proofVerificationAndReceiptNs.p95} | ${performance.proofVerificationAndReceiptNs.max} | ns / accepted case |
| Accepted corpus load | ${performance.acceptedCorpusLoadNs.p50} | ${performance.acceptedCorpusLoadNs.p95} | ${performance.acceptedCorpusLoadNs.max} | ns / 24 cases |
| Normalized export query | ${performance.normalizedQueryNsPerExport.p50} | ${performance.normalizedQueryNsPerExport.p95} | ${performance.normalizedQueryNsPerExport.max} | ns / lookup |

The current probe row measures the stable-v1 driver with a deterministic witness in a fresh process, realm, and module instance. The Phase 0 418-row distribution remains a historical ecosystem execution-envelope reference. Neither measurement is acceptance authority, and stable-v1 proposals are never promoted by coverage or probe non-observation.

## Offline ordinary analysis

Ordinary queries receive only \`AcceptedContractIndex\`: no raw sidecars, package code execution, network access, or query-time file reads. Artifact acquisition and receipt validation terminate before the analyzer-facing index is constructed.
`;
}

function loadSources() {
  return {
    ecosystem: json(INPUTS.ecosystem),
    historicalVerification: json(INPUTS.historicalVerification),
    manifest: json(INPUTS.manifest),
    syntheticCorpus: json(INPUTS.syntheticCorpus),
    phase13: json(INPUTS.phase13)
  };
}

function benchmark(path) {
  const child = spawnSync(path, [], { cwd: ROOT, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
  if (child.error) throw child.error;
  if (child.status !== 0) throw new Error(child.stderr || `benchmark exited ${child.status}`);
  return JSON.parse(child.stdout);
}

async function main() {
  const [mode, benchmarkPath] = process.argv.slice(2);
  const sources = loadSources();
  if (mode === "--write") {
    if (!benchmarkPath) throw new Error("--write requires the solid-contract-phase16-bench path");
    const accepted = benchmark(resolve(benchmarkPath));
    const probeExecution = await benchmarkCurrentProbeExecution();
    const report = buildPhase16Report({ ...sources, accepted, probeExecution });
    const refusals = buildRefusalReport(sources.ecosystem, sources.phase13);
    mkdirSync(PHASE16, { recursive: true });
    writeFileSync(REPORT_PATH, `${JSON.stringify(report, null, 2)}\n`);
    writeFileSync(REFUSALS_PATH, `${JSON.stringify(refusals, null, 2)}\n`);
    writeFileSync(REPORT_MARKDOWN_PATH, renderMarkdown(report));
    console.log(`wrote Phase 16 report for ${report.corpus.ecosystem.rows} ecosystem rows`);
    return;
  }
  if (mode === "--check") {
    const report = JSON.parse(readFileSync(REPORT_PATH, "utf8"));
    const refusals = JSON.parse(readFileSync(REFUSALS_PATH, "utf8"));
    assertPhase16Report(report, refusals, sources);
    assert.equal(readFileSync(REPORT_MARKDOWN_PATH, "utf8"), renderMarkdown(report));
    console.log(`checked Phase 16: ${report.corpus.ecosystem.generatablePercentage}% ecosystem, ${report.corpus.solidPrimitives.generatablePercentage}% Solid Primitives generatable`);
    return;
  }
  throw new Error("usage: bun scripts/package-contract-v2-phase16.mjs --check | --write <benchmark-bin>");
}

const isMain = process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) await main();
