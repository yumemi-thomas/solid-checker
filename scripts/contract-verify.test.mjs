// `solid-checker contract verify` -- RFC 0002 Stage 2, mechanical promotion to
// `verified`.
//
// Every blocker and every branch of the unknown-conversion rule is tested
// hermetically against a hand-written probe report: no install, no package
// code. The tests that write a contract need the native checker, because the
// promotion validates the document before it installs it; they skip when it is
// absent. The one integration test drives the whole pipeline -- generate is
// stood in for by a hand-authored draft, then probe --write, then verify --
// against a real installed Solid release, and skips cleanly offline.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

import { expandContract } from "../packages/cli/scripts/contract-document.mjs";
import { PROBE_MODES } from "../packages/cli/scripts/contract-probe-driver.mjs";
import {
  collectReviewItems,
  renderReviewPlanDocument,
  verifyReportPath
} from "../packages/cli/scripts/contract-review-plan.mjs";
import {
  collectBlockers,
  convertUnconfirmedClaims,
  dropInferredRowEvidence,
  pruneSummaryProbedMarkers,
  statedModes
} from "../packages/cli/scripts/contract-verification.mjs";
import { probeContract } from "../packages/cli/scripts/probe-contract.mjs";
import { verifyContract } from "../packages/cli/scripts/verify-contract.mjs";

const root = resolve(import.meta.dirname, "..");
const cli = join(root, "packages/cli/bin/solid-checker.mjs");
// `verifyContract` runs in-process and validates the document it is about to
// install with the native checker, so the launcher override has to be on this
// process's own environment. The checked-in bin/ binary lags rust/ source.
const native =
  process.env.SOLID_CHECKER_NATIVE_BIN ?? join(root, "rust/target/debug/solid-checker-rust");
if (existsSync(native)) process.env.SOLID_CHECKER_NATIVE_BIN = native;
const typeFacts = process.env.SOLID_TYPEFACTS_BIN ?? join(root, "bin/solid-typefacts");
const canWrite = existsSync(native);
const canGenerate = canWrite && existsSync(typeFacts);

const temporaries = [];
function workspace(prefix = "solid-checker-verify-") {
  const directory = mkdtempSync(join(tmpdir(), prefix));
  temporaries.push(directory);
  return directory;
}
process.on("exit", () => {
  for (const directory of temporaries) rmSync(directory, { recursive: true, force: true });
});

const sha256 = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const ALL_MODES = PROBE_MODES.map(mode => mode.name).sort();
const probedIn = (modes = ALL_MODES) => ({ kind: "probed", modes: [...modes].sort(), calls: 2 });

/// One contract exercising every arm of the conversion rule at once.
///
///   wrapMemo  every family-(B) claim probed in every stated mode -> survives
///   wrapRoot  a callback row the probe could not drive           -> converts
///   readsIt   family (A) only                                    -> passes through
///   project   a store-path return and an asyncBehavior           -> both convert
const CONTRACT = {
  schemaVersion: 1,
  package: { name: "verify-fixture", version: "1.0.0" },
  compilerFactsProtocol: 1,
  summaries: {
    "function-1": {
      kind: "function",
      callbacks: [{ parameter: 0, execution: "tracked", evidence: probedIn() }],
      returns: { kind: "accessor", label: "memo result", evidence: probedIn() },
      evidence: probedIn()
    },
    "function-2": {
      kind: "function",
      callbacks: [{ parameter: 0, execution: "inline", evidence: { kind: "inferred" } }],
      evidence: { kind: "inferred" }
    },
    "function-3": {
      kind: "function",
      reactiveReads: [{ kind: "parameter-member", parameter: 0, evidence: { kind: "inferred" } }],
      ownerRequirements: [{ operation: "effect", evidence: { kind: "inferred" } }],
      evidence: { kind: "inferred" }
    },
    "function-4": {
      kind: "function",
      returns: { kind: "store-path", label: "projection result", evidence: { kind: "inferred" } },
      asyncBehavior: "promise",
      evidence: { kind: "inferred" }
    }
  },
  entrypoints: {
    ".": {
      exports: {
        "function-1": ["wrapMemo"],
        "function-2": ["wrapRoot"],
        "function-3": ["readsIt"],
        "function-4": ["project"]
      }
    }
  },
  evidence: { kind: "inferred", generator: "solid-checker package generator" }
};

const expanded = document => {
  const contract = expandContract(structuredClone(document));
  for (const entry of Object.values(contract.entrypoints)) {
    for (const [name, summary] of Object.entries(entry.exports)) {
      entry.exports[name] = structuredClone(summary);
    }
  }
  return contract;
};

/// A passing `kind` observation for every export of the fixture.
///
/// Every export needs one, in every stated mode: `kind` is the one claim schema
/// v1 has no unknown sentinel for, so verification blocks on an unobserved one
/// rather than converting it. A report missing them is a report that observed
/// nothing about those exports.
const kindClaims = (names, modes = ALL_MODES) =>
  names.map(name => ({
    entrypoint: ".",
    export: name,
    claim: "kind=function",
    family: "B",
    status: "passed",
    modes: { attempted: [...modes].sort(), passed: [...modes].sort() }
  }));

/// The probe report the driver would have written for that contract after a
/// `--write`: the kind observations, three passing claims, three undriven ones
/// with real reasons.
function probeReport({ contractFile, hash, afterWrite = hash, overrides = {} } = {}) {
  return {
    schemaVersion: 1,
    package: { name: "verify-fixture", version: "1.0.0", installedVersion: "1.0.0" },
    contract: { path: contractFile, hash, afterWrite, markersWritten: 3 },
    identities: {
      generator: "solid-checker@test",
      probeDriver: "solid-checker@test",
      dialect: "solid-v1",
      runtime: { package: "solid-js", version: "1.9.14" }
    },
    modes: ALL_MODES,
    discovery: { enabled: true, parameters: [0, 1] },
    summary: { claims: 10, driven: 7, passed: 7, failed: 0, undriven: 3, incompleteness: 0 },
    claims: [
      ...kindClaims(["wrapMemo", "wrapRoot", "readsIt", "project"]),
      {
        entrypoint: ".",
        export: "wrapMemo",
        claim: "callbacks[0]=tracked",
        family: "B",
        status: "passed",
        modes: { attempted: ALL_MODES, passed: ALL_MODES }
      },
      {
        entrypoint: ".",
        export: "wrapMemo",
        claim: "returns=accessor",
        family: "B",
        status: "passed",
        modes: { attempted: ALL_MODES, passed: ALL_MODES }
      },
      {
        entrypoint: ".",
        export: "wrapRoot",
        claim: "callbacks[0]=inline",
        family: "B",
        status: "undriven",
        reason: "the synthesized call threw: TypeError: fn is not a function",
        modes: { attempted: ALL_MODES, passed: [] }
      },
      {
        entrypoint: ".",
        export: "project",
        claim: "returns=store-path",
        family: "C",
        status: "undriven",
        reason: "no generic store-path observation: confirming a store path means writing through the package's own setter",
        modes: { attempted: [], passed: [] }
      },
      {
        entrypoint: ".",
        export: "project",
        claim: "asyncBehavior",
        family: "C",
        status: "undriven",
        reason: "asyncBehavior has no evidence slot in schema v1, so a driven observation could not be recorded",
        modes: { attempted: [], passed: [] }
      }
    ],
    incompleteness: [],
    ...overrides
  };
}

/// A contract, the plan generation wrote beside it, and the probe report a
/// `contract probe --write` left. `plan` and `report` are hooks so a test can
/// break exactly one of them.
function draft({ document = CONTRACT, generation, plan: planOverride, report: reportOverride } = {}) {
  const directory = workspace();
  const contractFile = join(directory, "solid-reactivity.json");
  writeFileSync(contractFile, `${JSON.stringify(document, null, 2)}\n`);
  const hash = sha256(readFileSync(contractFile));
  const plan = {
    ...renderReviewPlanDocument(
      document.package.name,
      document.package.version,
      collectReviewItems(expandContract(structuredClone(document)).entrypoints),
      generation ?? { generator: "solid-checker@test", entrypoints: { ".": { modules: [] } } },
      hash
    ),
    ...planOverride
  };
  const planFile = join(directory, "solid-reactivity.review.json");
  writeFileSync(planFile, `${JSON.stringify(plan, null, 2)}\n`);
  const reportFile = join(directory, "solid-reactivity.probe.json");
  const report = { ...probeReport({ contractFile, hash }), ...reportOverride };
  writeFileSync(reportFile, `${JSON.stringify(report, null, 2)}\n`);
  return { directory, contractFile, planFile, reportFile, hash, plan, report };
}

function blockersFor(fixture, extra = {}) {
  return collectBlockers({
    contract: expanded(JSON.parse(readFileSync(fixture.contractFile, "utf8"))),
    contractHash: fixture.hash,
    contractPath: fixture.contractFile,
    report: fixture.report,
    reportPath: fixture.reportFile,
    plan: fixture.plan,
    planPath: fixture.planFile,
    reviewStatePath: join(fixture.directory, "solid-reactivity.review-state.json"),
    ...extra
  });
}

// ---------------------------------------------------------------- the blockers

test("a fully probed draft with a matching report raises no blocker", () => {
  assert.deepEqual(blockersFor(draft()), []);
});

test("a missing probe report blocks, because there is nothing that observed anything", () => {
  const fixture = draft();
  const blockers = blockersFor(fixture, { report: undefined });
  assert.equal(blockers.length, 1);
  assert.match(blockers[0], /no probe report at .*contract probe .* --write/s);
});

test("a probe report for other bytes blocks, on the same hash discipline as the plan", () => {
  const fixture = draft();
  const blockers = blockersFor(fixture, {
    report: { ...fixture.report, contract: { ...fixture.report.contract, afterWrite: "sha256:00" } }
  });
  assert.equal(blockers.length, 1);
  assert.match(blockers[0], /written for contract bytes sha256:00 .* re-probe these exact bytes/s);
});

test("a probe report that predates its own evidence write blocks", () => {
  // The hash matches -- it is a report of these bytes -- but no `--write`
  // happened, so none of the passing claims reached the contract and every one
  // of them would silently convert to unknown.
  const fixture = draft();
  const { afterWrite, markersWritten, ...contract } = fixture.report.contract;
  const blockers = blockersFor(fixture, { report: { ...fixture.report, contract } });
  assert.equal(blockers.length, 1);
  assert.match(blockers[0], /records 7 passed claim\(s\) but no evidence write/);
});

test("a failed probe blocks, and is never converted away", () => {
  const fixture = draft();
  const failed = {
    ...fixture.report,
    claims: fixture.report.claims.map(claim =>
      claim.claim === "callbacks[0]=tracked"
        ? { ...claim, status: "failed", reason: "observed inline" }
        : claim
    )
  };
  const blockers = blockersFor(fixture, { report: failed });
  assert.equal(blockers.length, 1);
  assert.match(blockers[0], /a probe failed: \.:wrapMemo callbacks\[0\]=tracked: observed inline/);
  assert.match(blockers[0], /converting the claim to unknown would hide/);
});

test("an incompleteness finding blocks: a negative a probe falsified is wrong, not incomplete", () => {
  const fixture = draft();
  const blockers = blockersFor(fixture, {
    report: {
      ...fixture.report,
      incompleteness: [
        {
          entrypoint: ".",
          export: "readsIt",
          claim: "callbacks[0]=tracked",
          mode: "client",
          text: ".:readsIt invoked the callback passed at parameter 0 in client (observed tracked), and the contract states no such claim"
        }
      ]
    }
  });
  assert.equal(blockers.length, 1);
  assert.match(blockers[0], /an incompleteness finding contradicts a negative claim/);
});

test("a kind claim nothing observed blocks, because there is no sentinel to convert it to", () => {
  const fixture = draft();
  const withoutKinds = {
    ...fixture.report,
    claims: fixture.report.claims.filter(claim => !claim.claim.startsWith("kind="))
  };
  const blockers = blockersFor(fixture, { report: withoutKinds });
  assert.equal(blockers.length, 1);
  assert.match(blockers[0], /no passing kind observation for 4 export\(s\)/);
  assert.match(blockers[0], /wrapMemo \(client, server, development, production\)/);
  assert.match(blockers[0], /no unknown sentinel for/);
});

test("a kind claim observed in fewer modes than the export is stated for blocks", () => {
  const fixture = draft();
  const narrowed = {
    ...fixture.report,
    modes: ["client"],
    claims: fixture.report.claims.map(claim =>
      claim.claim.startsWith("kind=")
        ? { ...claim, modes: { attempted: ["client"], passed: ["client"] } }
        : claim
    )
  };
  const blockers = blockersFor(fixture, { report: narrowed });
  assert.equal(blockers.length, 1);
  assert.match(blockers[0], /server, development, production/);
});

test("a package the probe could not import cannot be machine-verified at all", () => {
  // The consequence of the kind rule, stated as its own case because it is the
  // one that used to certify a contract none of whose claims were observed.
  const fixture = draft();
  const importFailed = {
    ...fixture.report,
    summary: { claims: 10, driven: 0, passed: 0, failed: 0, undriven: 10, incompleteness: 0 },
    claims: fixture.report.claims.map(claim => ({
      ...claim,
      status: "undriven",
      reason: "import of verify-fixture threw: Error: refuses to load in this environment",
      modes: { attempted: ALL_MODES, passed: [] }
    }))
  };
  const blockers = blockersFor(fixture, { report: importFailed });
  assert.equal(blockers.length, 1);
  assert.match(blockers[0], /no passing kind observation for 4 export\(s\)/);
});

test("a probe report produced with discovery disabled is refused", () => {
  const fixture = draft();
  const noDiscovery = {
    ...fixture.report,
    discovery: { enabled: false, parameters: [] }
  };
  const blockers = blockersFor(fixture, { report: noDiscovery });
  assert.equal(blockers.length, 1);
  assert.match(blockers[0], /produced with discovery disabled \(--no-discovery\)/);
  assert.match(blockers[0], /the only automated check that can contradict one/);
});

test("a probe report that records no discovery state at all is refused, not assumed", () => {
  const fixture = draft();
  const { discovery, ...silent } = fixture.report;
  const blockers = blockersFor(fixture, { report: silent });
  assert.equal(blockers.length, 1);
  assert.match(blockers[0], /records no discovery state/);
});

test("a closure note on an emitted entrypoint blocks outright", () => {
  const fixture = draft({
    generation: {
      generator: "solid-checker@test",
      entrypoints: {
        ".": { modules: [], notes: ["closure could not be fully enumerated: ./impl"] }
      }
    }
  });
  const blockers = blockersFor(fixture);
  assert.equal(blockers.length, 1);
  assert.match(blockers[0], /\. carries a closure note: closure could not be fully enumerated/);
  assert.match(blockers[0], /declines to claim it enumerated/);
});

test("a closure note on an entrypoint the contract does not emit does not block", () => {
  // A refused entrypoint is absent from the contract, so a consumer importing
  // it already gets an explicit uncertifiable result rather than a wrong claim.
  const fixture = draft({
    generation: {
      generator: "solid-checker@test",
      entrypoints: {
        ".": { modules: [] },
        "./refused": { modules: [], notes: ["closure could not be fully enumerated"] }
      }
    }
  });
  assert.deepEqual(blockersFor(fixture), []);
});

test("a plan written for other bytes blocks, because its closure notes are about them", () => {
  const fixture = draft();
  const blockers = blockersFor(fixture, { plan: { ...fixture.plan, contract: "sha256:00" } });
  assert.equal(blockers.length, 1);
  assert.match(blockers[0], /review plan .* was written for contract bytes sha256:00/);
});

test("a missing plan blocks: nothing else records whether the closure was enumerated", () => {
  const fixture = draft();
  const blockers = blockersFor(fixture, { plan: undefined });
  assert.equal(blockers.length, 1);
  assert.match(blockers[0], /no review plan at .*closure-note blocker/s);
});

test("a review already under way blocks, because verification moves its bytes", () => {
  const fixture = draft();
  assert.match(
    blockersFor(fixture, {
      reviewState: { resolutions: { "some-item": { decision: "confirm" } } }
    })[0],
    /already records 1 review decision/
  );
  assert.match(
    blockersFor(fixture, { reviewState: { resolutions: {}, promoted: { evidence: "reviewed" } } })[0],
    /already records a promotion to reviewed evidence/
  );
});

// ------------------------------------------------------- the conversion rule

test("stated modes come from the document, never from what the probe happened to attempt", () => {
  assert.deepEqual(statedModes({ conditions: [] }), [
    "client",
    "server",
    "development",
    "production"
  ]);
  assert.deepEqual(statedModes({ conditions: ["browser"] }), [
    "client",
    "development",
    "production"
  ]);
  assert.deepEqual(statedModes({ conditions: [] }, ["development"]), ["development"]);
});

test("a probed row survives and an unprobed one converts its whole domain", () => {
  const fixture = draft();
  const { contract, conversions, probed } = convertUnconfirmedClaims(
    expanded(CONTRACT),
    fixture.report
  );
  const exports_ = contract.entrypoints["."].exports;

  assert.equal(exports_.wrapMemo.callbacks[0].evidence.kind, "probed");
  assert.equal(exports_.wrapMemo.returns.evidence.kind, "probed");
  assert.deepEqual(exports_.wrapRoot.callbacks, { status: "unknown" });
  assert.deepEqual(exports_.project.returns, { status: "unknown" });
  assert.deepEqual(exports_.project.asyncBehavior, { status: "unknown" });

  // Family (A) passes through untouched: the generator emits the sentinel
  // where the compiler facts are not exact, so an emitted row is the proven
  // case.
  assert.equal(exports_.readsIt.reactiveReads.length, 1);
  assert.equal(exports_.readsIt.ownerRequirements.length, 1);
  // And `kind` is never converted -- there is no sentinel for it, and a runtime
  // kind that disagreed would have been a failed probe, which blocks.
  assert.equal(exports_.project.kind, "function");

  assert.equal(probed.length, 2);
  assert.deepEqual(
    conversions.map(conversion => `${conversion.export}.${conversion.field}`).sort(),
    ["project.asyncBehavior", "project.returns", "wrapRoot.callbacks"]
  );
});

test("a conversion records the claim identity, the value the machine held, and the reason", () => {
  const fixture = draft();
  const { conversions } = convertUnconfirmedClaims(expanded(CONTRACT), fixture.report);
  const callbacks = conversions.find(conversion => conversion.field === "callbacks");
  assert.deepEqual(callbacks.claimed, [
    { parameter: 0, execution: "inline", evidence: { kind: "inferred" } }
  ]);
  assert.deepEqual(callbacks.claims, [
    {
      claim: "callbacks[0]=inline",
      reason: "the synthesized call threw: TypeError: fn is not a function"
    }
  ]);
  assert.deepEqual(callbacks.modes, ["client", "server", "development", "production"]);
  const asyncBehavior = conversions.find(conversion => conversion.field === "asyncBehavior");
  assert.equal(asyncBehavior.claimed, "promise");
  assert.match(asyncBehavior.claims[0].reason, /no evidence slot in schema v1/);
});

test("one unconfirmable row converts every row of its domain, because the sentinel is a field", () => {
  const document = structuredClone(CONTRACT);
  document.summaries["function-1"].callbacks.push({
    parameter: 1,
    execution: "deferred",
    evidence: { kind: "inferred" }
  });
  const { contract, conversions } = convertUnconfirmedClaims(expanded(document), probeReport({}));
  assert.deepEqual(contract.entrypoints["."].exports.wrapMemo.callbacks, { status: "unknown" });
  const converted = conversions.find(conversion => conversion.export === "wrapMemo");
  assert.deepEqual(converted.claims.map(claim => claim.claim), [
    "callbacks[0]=tracked",
    "callbacks[1]=deferred"
  ]);
  // The probed row is gone with the field, and reported as lost rather than
  // silently kept alongside a sentinel that would contradict it.
  assert.equal(converted.claimed[0].evidence.kind, "probed");
});

test("probed evidence that does not cover every stated mode converts the domain", () => {
  const document = structuredClone(CONTRACT);
  document.summaries["function-1"].callbacks[0].evidence = probedIn(["client", "development"]);
  const { contract, conversions } = convertUnconfirmedClaims(expanded(document), probeReport({}));
  assert.deepEqual(contract.entrypoints["."].exports.wrapMemo.callbacks, { status: "unknown" });
  assert.match(
    conversions.find(conversion => conversion.field === "callbacks").claims[0].reason,
    /does not cover every mode the claim is stated for \(client, server, development, production\)/
  );
});

test("the same evidence covers an entrypoint whose conditions state fewer modes", () => {
  const document = structuredClone(CONTRACT);
  document.summaries["function-1"].callbacks[0].evidence = probedIn([
    "client",
    "development",
    "production"
  ]);
  document.summaries["function-1"].returns.evidence = probedIn([
    "client",
    "development",
    "production"
  ]);
  document.entrypoints["."].conditions = ["browser"];
  const { contract } = convertUnconfirmedClaims(expanded(document), probeReport({}));
  assert.equal(contract.entrypoints["."].exports.wrapMemo.callbacks[0].evidence.kind, "probed");
});

test("an owner row, a callback argument descriptor, and a return leaf each convert", () => {
  const document = structuredClone(CONTRACT);
  document.summaries["function-1"].callbacks[0].owner = "created";
  const owned = convertUnconfirmedClaims(expanded(document), probeReport({}));
  assert.match(
    owned.conversions.find(conversion => conversion.field === "callbacks").claims[0].reason,
    /owner rows have no probe form/
  );

  const withArguments = structuredClone(CONTRACT);
  withArguments.summaries["function-1"].callbacks[0].arguments = [
    { kind: "accessor", label: "value" }
  ];
  assert.match(
    convertUnconfirmedClaims(expanded(withArguments), probeReport({})).conversions.find(
      conversion => conversion.field === "callbacks"
    ).claims[0].reason,
    /callback argument descriptors have no probe form/
  );

  const nested = structuredClone(CONTRACT);
  nested.summaries["function-1"].returns = {
    kind: "accessor",
    label: "memo result",
    evidence: probedIn(),
    properties: { inner: { kind: "accessor", label: "inner" } }
  };
  assert.match(
    convertUnconfirmedClaims(expanded(nested), probeReport({})).conversions.find(
      conversion => conversion.field === "returns"
    ).claims[0].reason,
    /return leaves have no probe form/
  );
});

test("an inherited row converts: the tier of the contract it came from is not checkable here", () => {
  const document = structuredClone(CONTRACT);
  document.summaries["function-1"].callbacks[0].evidence = {
    kind: "inherited-from",
    package: "upstream",
    version: "2.1.0"
  };
  document.summaries["function-3"].reactiveReads[0].evidence = {
    kind: "inherited-from",
    package: "upstream",
    version: "2.1.0"
  };
  const { contract, conversions } = convertUnconfirmedClaims(expanded(document), probeReport({}));
  assert.deepEqual(contract.entrypoints["."].exports.wrapMemo.callbacks, { status: "unknown" });
  assert.deepEqual(contract.entrypoints["."].exports.readsIt.reactiveReads, { status: "unknown" });
  assert.match(
    conversions.find(conversion => conversion.export === "readsIt").claims[0].reason,
    /inherited from upstream@2\.1\.0/
  );
});

test("a variant's claims are judged against the modes its own conditions resolve under", () => {
  const document = structuredClone(CONTRACT);
  document.summaries["function-1"] = {
    kind: "function",
    variants: [
      {
        conditions: ["development"],
        summary: {
          kind: "function",
          callbacks: [{ parameter: 0, execution: "tracked", evidence: probedIn(["development"]) }]
        }
      },
      {
        conditions: ["production"],
        summary: {
          kind: "function",
          callbacks: [{ parameter: 0, execution: "inline", evidence: { kind: "inferred" } }]
        }
      }
    ]
  };
  const { contract, conversions } = convertUnconfirmedClaims(expanded(document), probeReport({}));
  const wrapMemo = contract.entrypoints["."].exports.wrapMemo;
  assert.equal(wrapMemo.variants[0].summary.callbacks[0].evidence.kind, "probed");
  assert.deepEqual(wrapMemo.variants[1].summary.callbacks, { status: "unknown" });
  assert.equal(
    conversions.find(conversion => conversion.field === "variants[1].summary.callbacks").modes[0],
    "production"
  );
});

test("a probed marker the consumed report does not witness converts like an unprobed row", () => {
  // The stale-marker attack, end to end at the conversion rule. A healthy probe
  // wrote `probed` markers; a later run observed nothing at all -- here because
  // the import threw -- and the markers stayed in the document. Verification
  // used to read them as this run's observation.
  const fixture = draft();
  const blind = {
    ...fixture.report,
    summary: { claims: 10, driven: 0, passed: 0, failed: 0, undriven: 10, incompleteness: 0 },
    claims: fixture.report.claims.map(claim => ({
      ...claim,
      status: "undriven",
      reason: "import of verify-fixture threw: Error: refuses to load in this environment",
      modes: { attempted: ALL_MODES, passed: [] }
    }))
  };
  const { contract, conversions, staleMarkers } = convertUnconfirmedClaims(
    expanded(CONTRACT),
    blind
  );
  const exports_ = contract.entrypoints["."].exports;
  assert.deepEqual(exports_.wrapMemo.callbacks, { status: "unknown" });
  assert.deepEqual(exports_.wrapMemo.returns, { status: "unknown" });
  const reason = conversions.find(conversion => conversion.field === "callbacks").claims[0].reason;
  // The probe report's own reason wins, because it says more than the generic
  // one; the marker itself is reported separately as unwitnessed.
  assert.match(reason, /import of verify-fixture threw/);
  assert.deepEqual(
    staleMarkers.map(marker => `${marker.export}.${marker.field}`).sort(),
    ["wrapMemo.callbacks[0]", "wrapMemo.evidence", "wrapMemo.returns"]
  );
  assert.equal(staleMarkers[0].marker.kind, "probed");
});

test("a probed marker witnessed only in fewer modes than it asserts converts", () => {
  // The `--modes` narrowing shape: the marker claims four modes, this run's
  // report passed the claim in one.
  const narrowed = {
    ...probeReport({}),
    modes: ["client"],
    claims: probeReport({}).claims.map(claim =>
      claim.status === "passed"
        ? { ...claim, modes: { attempted: ["client"], passed: ["client"] } }
        : claim
    )
  };
  const { contract, staleMarkers } = convertUnconfirmedClaims(expanded(CONTRACT), narrowed);
  assert.deepEqual(contract.entrypoints["."].exports.wrapMemo.callbacks, { status: "unknown" });
  assert.equal(
    staleMarkers.some(marker => marker.claim === "callbacks[0]=tracked"),
    true
  );
});

test("a summary-level probed marker does not outlive the claims it covered", () => {
  const document = structuredClone(CONTRACT);
  // `wrapRoot` carries a summary marker but its only probeable claim converts,
  // so the marker asserts an observation of nothing -- and a row with no
  // evidence of its own would inherit it.
  document.summaries["function-2"].evidence = probedIn();
  const { contract } = convertUnconfirmedClaims(expanded(document), probeReport({}));
  const exports_ = contract.entrypoints["."].exports;
  assert.deepEqual(exports_.wrapRoot.callbacks, { status: "unknown" });
  assert.equal(exports_.wrapRoot.evidence, undefined);
  // The one whose claims survived keeps its marker.
  assert.equal(exports_.wrapMemo.evidence.kind, "probed");
});

test("the summary-marker prune is the same rule on the review promotion's side", () => {
  // What `contract review --promote reviewed` calls after it deletes the
  // claims a reviewer certified absent. The shape is the one a review can
  // actually produce: the export's only probeable claim is gone and the
  // summary marker would otherwise still assert an observation of it, which
  // every row without evidence of its own inherits.
  const entrypoints = {
    ".": {
      exports: {
        emptied: { kind: "function", evidence: probedIn() },
        intact: {
          kind: "function",
          callbacks: [{ parameter: 0, execution: "inline", evidence: probedIn() }],
          evidence: probedIn()
        },
        partly: {
          kind: "function",
          callbacks: [{ parameter: 0, execution: "inline", evidence: probedIn() }],
          returns: { kind: "accessor", label: "memo", evidence: { kind: "inferred" } },
          evidence: probedIn()
        },
        nested: {
          kind: "function",
          evidence: probedIn(),
          variants: [{ conditions: ["development"], summary: { kind: "function", evidence: probedIn() } }]
        }
      }
    }
  };
  // Four: `emptied`, `partly`, `nested`, and `nested`'s variant summary. The
  // walk descends into variants, which carry their own markers.
  assert.equal(pruneSummaryProbedMarkers(entrypoints), 4);
  const exports_ = entrypoints["."].exports;
  assert.equal(exports_.emptied.evidence, undefined, "nothing left for the marker to be about");
  assert.equal(exports_.intact.evidence.kind, "probed", "its one claim is still observed");
  assert.equal(
    exports_.partly.evidence,
    undefined,
    "one covered claim without a probed marker is enough: the summary marker was written for both"
  );
  assert.equal(exports_.nested.evidence, undefined);
  assert.equal(exports_.nested.variants[0].summary.evidence, undefined);
  // Idempotent, so running it in both paths costs nothing.
  assert.equal(pruneSummaryProbedMarkers(entrypoints), 0);
});

test("every domain an inherited summary's variants carry converts too", () => {
  // The top-level domains of an inherited summary already converted; the walk
  // then descended into `variants` on their own evidence and let the exact
  // per-environment claims through -- which are the ones a consumer actually
  // selects.
  const document = structuredClone(CONTRACT);
  document.summaries["function-1"] = {
    kind: "function",
    callbacks: [{ parameter: 0, execution: "tracked", evidence: probedIn() }],
    evidence: { kind: "inherited-from", package: "upstream", version: "2.1.0" },
    variants: [
      {
        conditions: ["development"],
        summary: {
          kind: "function",
          callbacks: [{ parameter: 0, execution: "tracked", evidence: probedIn(["development"]) }],
          reactiveReads: [{ kind: "parameter-member", parameter: 0 }],
          asyncBehavior: "promise"
        }
      }
    ]
  };
  const { contract, conversions } = convertUnconfirmedClaims(expanded(document), probeReport({}));
  const wrapMemo = contract.entrypoints["."].exports.wrapMemo;
  assert.deepEqual(wrapMemo.callbacks, { status: "unknown" });
  assert.deepEqual(wrapMemo.variants[0].summary.callbacks, { status: "unknown" });
  assert.deepEqual(wrapMemo.variants[0].summary.reactiveReads, { status: "unknown" });
  assert.deepEqual(wrapMemo.variants[0].summary.asyncBehavior, { status: "unknown" });
  assert.match(
    conversions.find(
      conversion => conversion.field === "variants[0].summary.reactiveReads"
    ).claims[0].reason,
    /inherited from upstream@2\.1\.0/
  );
});

test("nothing inferred survives a promotion, because certification rejects it", () => {
  const fixture = draft();
  const { contract } = convertUnconfirmedClaims(expanded(CONTRACT), fixture.report);
  const dropped = dropInferredRowEvidence(contract.entrypoints);
  assert.equal(dropped > 0, true);
  assert.equal(
    JSON.stringify(contract.entrypoints).includes('"inferred"'),
    false,
    "an inferred row inside a certifying document is one the loader refuses"
  );
  // A row with no evidence of its own inherits the document's; a probed one is
  // left exactly as it is.
  assert.equal(
    contract.entrypoints["."].exports.wrapMemo.callbacks[0].evidence.kind,
    "probed"
  );
  assert.equal(contract.entrypoints["."].exports.readsIt.reactiveReads[0].evidence, undefined);
});

// -------------------------------------------------------------- the command

test("verify promotes, writes the sidecar, and re-binds the plan", { skip: !canWrite }, async () => {
  const fixture = draft();
  const report = await verifyContract([fixture.contractFile]);
  assert.equal(process.exitCode ?? 0, 0);

  const written = JSON.parse(readFileSync(fixture.contractFile, "utf8"));
  assert.deepEqual(written.evidence, { kind: "verified" });
  const exports_ = expandContract(written).entrypoints["."].exports;
  assert.equal(exports_.wrapMemo.callbacks[0].evidence.kind, "probed");
  assert.deepEqual(exports_.wrapRoot.callbacks, { status: "unknown" });
  assert.deepEqual(exports_.project.asyncBehavior, { status: "unknown" });

  const sidecar = JSON.parse(readFileSync(verifyReportPath(fixture.contractFile), "utf8"));
  assert.deepEqual(sidecar, report);
  assert.equal(sidecar.schemaVersion, 1);
  assert.equal(sidecar.contract.before, fixture.hash);
  assert.equal(sidecar.contract.after, sha256(readFileSync(fixture.contractFile)));
  assert.equal(sidecar.probeReport.contract, fixture.hash);
  assert.equal(sidecar.identities.generator, "solid-checker@test");
  assert.match(sidecar.identities.verifier, /^solid-checker@/);
  assert.equal(sidecar.identities.dialect, "solid-v1");
  assert.equal(sidecar.summary.conversions, 3);
  assert.equal(sidecar.summary.probedRows, 2);
  assert.equal(sidecar.blockers.raised.length, 0);
  assert.equal(sidecar.blockers.checked.includes("closure-note"), true);

  // The plan follows the bytes, so `contract review` can still be run on the
  // promoted document -- which is what makes verified -> reviewed reachable.
  const plan = JSON.parse(readFileSync(fixture.planFile, "utf8"));
  assert.equal(plan.contract, sidecar.contract.after);
  assert.equal(
    plan.items.some(
      item => item.kind === "unknown-sentinel" && item.text === ".:wrapRoot: callbacks"
    ),
    true,
    "a converted domain is a new question for the reviewed tier"
  );
  process.exitCode = 0;
});

test("the plan rewrite keeps every because and gives the conversions one", { skip: !canWrite }, async () => {
  // `because` is the only place a reviewer learns *why* a claim is unknown, and
  // a contract document carries none of it. Re-deriving the plan from the
  // verified bytes therefore destroyed it -- for the sentinels generation
  // attributed, and for the ones verification had just created.
  const document = structuredClone(CONTRACT);
  document.summaries["function-3"].reactiveReads = { status: "unknown" };
  const fixture = draft({ document });
  const generated = JSON.parse(readFileSync(fixture.planFile, "utf8"));
  const attributed = generated.items.find(
    item => item.kind === "unknown-sentinel" && item.target.export === "readsIt"
  );
  attributed.because = {
    attributions: [{ obligation: "UnresolvedDispatch", mechanism: "enclosing-chain" }]
  };
  writeFileSync(fixture.planFile, `${JSON.stringify(generated, null, 2)}\n`);

  await verifyContract([fixture.contractFile]);
  assert.equal(process.exitCode ?? 0, 0);
  const plan = JSON.parse(readFileSync(fixture.planFile, "utf8"));

  // Carried forward by id: an unchanged question keeps its identity, so it
  // keeps its reason.
  const carried = plan.items.find(item => item.id === attributed.id);
  assert.deepEqual(carried.because.attributions, attributed.because.attributions);

  // And a sentinel this verification produced gets one of its own, mirrored
  // from the conversion record the sidecar already carries.
  const converted = plan.items.find(
    item => item.kind === "unknown-sentinel" && item.text === ".:wrapRoot: callbacks"
  );
  assert.equal(converted.because.conversion.by, "contract verify");
  assert.deepEqual(converted.because.conversion.modes, [
    "client",
    "server",
    "development",
    "production"
  ]);
  assert.match(converted.because.conversion.claims[0].reason, /the synthesized call threw/);
  assert.equal(converted.because.conversion.claims[0].claim, "callbacks[0]=inline");
  process.exitCode = 0;
});

test("verifying twice is a no-op rather than a hash refusal", { skip: !canWrite }, async () => {
  const fixture = draft();
  await verifyContract([fixture.contractFile]);
  const after = readFileSync(fixture.contractFile, "utf8");
  const again = await verifyContract([fixture.contractFile]);
  assert.equal(process.exitCode ?? 0, 0);
  assert.equal(readFileSync(fixture.contractFile, "utf8"), after);
  assert.equal(again.contract.after, sha256(Buffer.from(after, "utf8")));
  process.exitCode = 0;
});

test("a blocked verification leaves the contract, plan and sidecar untouched", { skip: !canWrite }, async () => {
  const fixture = draft({
    generation: {
      generator: "solid-checker@test",
      entrypoints: { ".": { modules: [], notes: ["unreadable module bytes"] } }
    }
  });
  const before = readFileSync(fixture.contractFile, "utf8");
  const planBefore = readFileSync(fixture.planFile, "utf8");
  await verifyContract([fixture.contractFile]);
  assert.equal(process.exitCode, 1);
  assert.equal(readFileSync(fixture.contractFile, "utf8"), before);
  assert.equal(readFileSync(fixture.planFile, "utf8"), planBefore);
  assert.equal(existsSync(verifyReportPath(fixture.contractFile)), false);
  process.exitCode = 0;
});

test("verify refuses a contract that already carries a stronger claim", { skip: !canWrite }, async () => {
  const document = structuredClone(CONTRACT);
  document.evidence = { kind: "reviewed" };
  const fixture = draft({ document });
  const before = readFileSync(fixture.contractFile, "utf8");
  await verifyContract([fixture.contractFile]);
  assert.equal(process.exitCode, 1);
  assert.equal(readFileSync(fixture.contractFile, "utf8"), before);
  process.exitCode = 0;
});

test("a refused entrypoint and an unbindable artifact do not block", { skip: !canWrite }, async () => {
  const fixture = draft();
  const plan = JSON.parse(readFileSync(fixture.planFile, "utf8"));
  plan.items = [
    {
      id: "refused-entrypoint-aaaaaaaaaaaa",
      kind: "refused-entrypoint",
      target: { entrypoint: "./server" },
      text: "./server: runtime target is not analyzable"
    },
    {
      id: "artifact-binding-bbbbbbbbbbbb",
      kind: "artifact-binding",
      target: { field: "artifacts.implementation" },
      text: "contract emitted outside the package: no artifact hash can be recorded"
    },
    ...plan.items
  ];
  writeFileSync(fixture.planFile, `${JSON.stringify(plan, null, 2)}\n`);
  await verifyContract([fixture.contractFile]);
  assert.equal(process.exitCode ?? 0, 0);
  assert.equal(
    JSON.parse(readFileSync(fixture.contractFile, "utf8")).evidence.kind,
    "verified"
  );
  // Both survive the plan rewrite: no contract document witnesses them, so
  // re-deriving the plan from the verified bytes would otherwise lose them.
  const rewritten = JSON.parse(readFileSync(fixture.planFile, "utf8"));
  assert.deepEqual(
    rewritten.items.slice(0, 2).map(item => item.kind),
    ["refused-entrypoint", "artifact-binding"]
  );
  process.exitCode = 0;
});

test("verified composes with a human review of what is left", { skip: !canWrite }, () => {
  const fixture = draft();
  const runCli = args =>
    spawnSync(process.execPath, [cli, ...args], {
      encoding: "utf8",
      env: { ...process.env, SOLID_CHECKER_NATIVE_BIN: native }
    });

  const verified = runCli(["contract", "verify", fixture.contractFile]);
  assert.equal(verified.status ?? 0, 0, verified.stdout + verified.stderr);
  assert.match(verified.stdout, /converted \.:wrapRoot callbacks to unknown/);

  // Listing a verified contract does not report it as unfinished: it certifies
  // already, and its items are the optional upgrade.
  const listed = runCli(["contract", "review", fixture.contractFile]);
  assert.equal(listed.status, 0, listed.stdout + listed.stderr);
  assert.match(listed.stdout, /evidence verified/);
  assert.match(listed.stdout, /already certifies as verified/);

  const plan = JSON.parse(readFileSync(fixture.planFile, "utf8"));
  const answers = join(fixture.directory, "answers.json");
  writeFileSync(
    answers,
    `${JSON.stringify(
      Object.fromEntries(
        plan.items.map(item => [
          item.id,
          item.kind === "unknown-sentinel" || item.kind === "no-callback-row" ? "absent" : "confirm"
        ])
      )
    )}\n`
  );
  const resolved = runCli(["contract", "review", fixture.contractFile, "--answers", answers]);
  assert.notEqual(resolved.status, 2, resolved.stdout + resolved.stderr);
  const promoted = runCli([
    "contract",
    "review",
    fixture.contractFile,
    "--promote",
    "reviewed"
  ]);
  assert.equal(promoted.status, 0, promoted.stdout + promoted.stderr);
  const document = JSON.parse(readFileSync(fixture.contractFile, "utf8"));
  assert.equal(document.evidence.kind, "reviewed");
  // The probed rows the machine earned survive the human promotion on top.
  assert.equal(
    expandContract(document).entrypoints["."].exports.wrapMemo.callbacks[0].evidence.kind,
    "probed"
  );
});

test("contract review refuses --promote verified and names the command that does it", () => {
  const fixture = draft();
  const child = spawnSync(
    process.execPath,
    [cli, "contract", "review", fixture.contractFile, "--promote", "verified"],
    { encoding: "utf8" }
  );
  assert.equal(child.status, 2);
  assert.match(child.stderr, /solid-checker contract verify <contract>/);
  assert.match(child.stderr, /takes no decision at all/);
});

test("a review cannot be transferred from a machine-verified contract", { skip: !canWrite }, async () => {
  const previous = draft();
  await verifyContract([previous.contractFile]);
  process.exitCode = 0;
  const next = draft();
  const child = spawnSync(
    process.execPath,
    [cli, "contract", "review", next.contractFile, "--transfer-from", previous.contractFile],
    { encoding: "utf8", env: { ...process.env, SOLID_CHECKER_NATIVE_BIN: native } }
  );
  assert.equal(child.status, 2);
  assert.match(child.stderr, /a verification is reproduced, never transferred/);
  assert.match(child.stderr, /contract probe .* --write && solid-checker contract verify/);
});

test("the CLI dispatches contract verify", () => {
  const child = spawnSync(process.execPath, [cli, "contract", "verify", "--help"], {
    encoding: "utf8"
  });
  assert.equal(child.status ?? 0, 0);
  assert.match(child.stdout, /solid-checker contract verify <CONTRACT>/);
  assert.match(child.stdout, /A verified contract is not a reviewed one/);
});

// ------------------------------------------------------------- the pipeline

/// generate -> probe --write -> verify, against a real installed Solid release.
///
/// It skips when the install cannot happen -- offline, or no npm -- and when the
/// native checker is absent, since the promotion validates before it installs.
test("the pipeline runs end to end against an installed Solid release", async t => {
  if (!canWrite) {
    t.skip(`no native solid-checker at ${native}`);
    return;
  }
  const directory = workspace("solid-checker-verify-install-");
  writeFileSync(
    join(directory, "package.json"),
    JSON.stringify({ name: "verify-integration", version: "1.0.0", private: true })
  );
  const install = spawnSync(
    "npm",
    ["install", "--prefix", directory, "--no-audit", "--no-fund", "--no-save", "solid-js@1.9.14"],
    { encoding: "utf8", timeout: 300_000 }
  );
  if (install.status !== 0) {
    t.skip(
      `could not install solid-js@1.9.14: ${(install.stderr ?? install.error?.message ?? "").trim()}`
    );
    return;
  }
  const packageRoot = join(directory, "node_modules", "pipeline-fixture");
  mkdirSync(packageRoot, { recursive: true });
  writeFileSync(
    join(packageRoot, "package.json"),
    JSON.stringify({
      name: "pipeline-fixture",
      version: "1.0.0",
      type: "module",
      exports: { ".": "./index.js" }
    })
  );
  writeFileSync(
    join(packageRoot, "index.js"),
    [
      'import { createMemo, createRoot } from "solid-js";',
      "export const wrapMemo = compute => createMemo(compute);",
      "export const wrapRoot = body => createRoot(() => body());",
      // Undrivable by construction: nothing in schema v1 says what parameter 0
      // is, so the driver passes `undefined` and the call refuses it.
      "export const needsOptions = (options, body) => body(options.value);",
      ""
    ].join("\n")
  );

  // The contract states its claims for the browser conditions only, so the
  // modes a claim must be probed in are the three the driver can reach here.
  const contract = {
    schemaVersion: 1,
    package: { name: "pipeline-fixture", version: "1.0.0" },
    compilerFactsProtocol: 1,
    summaries: {
      "function-1": {
        kind: "function",
        callbacks: [{ parameter: 0, execution: "tracked" }],
        returns: { kind: "accessor", label: "memo result" }
      },
      "function-2": { kind: "function", callbacks: [{ parameter: 0, execution: "inline" }] },
      "function-3": { kind: "function", callbacks: [{ parameter: 1, execution: "inline" }] }
    },
    entrypoints: {
      ".": {
        conditions: ["browser"],
        exports: {
          "function-1": ["wrapMemo"],
          "function-2": ["wrapRoot"],
          "function-3": ["needsOptions"]
        }
      }
    },
    evidence: { kind: "inferred", generator: "solid-checker package generator" }
  };
  const contractFile = join(directory, "solid-reactivity.json");
  writeFileSync(contractFile, `${JSON.stringify(contract, null, 2)}\n`);
  writeFileSync(
    join(directory, "solid-reactivity.review.json"),
    `${JSON.stringify(
      renderReviewPlanDocument(
        "pipeline-fixture",
        "1.0.0",
        collectReviewItems(expandContract(structuredClone(contract)).entrypoints),
        { generator: "solid-checker@test", entrypoints: { ".": { modules: [] } } },
        sha256(readFileSync(contractFile))
      ),
      null,
      2
    )}\n`
  );

  const probed = await probeContract([contractFile, "--write"]);
  process.exitCode = 0;
  assert.equal(probed.summary.failed, 0, JSON.stringify(probed.claims, null, 2));
  assert.equal(probed.summary.incompleteness, 0);

  const verified = await verifyContract([contractFile]);
  assert.equal(process.exitCode ?? 0, 0);
  process.exitCode = 0;

  const document = JSON.parse(readFileSync(contractFile, "utf8"));
  assert.deepEqual(document.evidence, { kind: "verified" });
  const exports_ = expandContract(document).entrypoints["."].exports;

  // The claims the probe drove keep their probed evidence, with the measured
  // call count: `wrapMemo` is invoked once, because the call-site memo caches
  // and the accessor read happens in a memo of the driver's own.
  assert.deepEqual(exports_.wrapMemo.callbacks[0].evidence, {
    kind: "probed",
    modes: ["client", "development", "production"],
    calls: 1
  });
  assert.equal(exports_.wrapRoot.callbacks[0].evidence.kind, "probed");

  // The one the driver could not construct a call for became the sentinel, and
  // the loss is recorded outside the contract with the reason.
  assert.deepEqual(exports_.needsOptions.callbacks, { status: "unknown" });
  const conversion = verified.conversions.find(entry => entry.export === "needsOptions");
  assert.deepEqual(conversion.claims.map(claim => claim.claim), ["callbacks[1]=inline"]);
  assert.match(conversion.claims[0].reason, /the synthesized call threw/);
  assert.deepEqual(conversion.claimed[0].parameter, 1);
  assert.deepEqual(conversion.modes, ["client", "development", "production"]);

  // Nothing inferred survives, which is what certification requires of a
  // document carrying row evidence at all.
  assert.equal(JSON.stringify(document).includes('"inferred"'), false);

  // The consumer side: the document the loader is asked to trust validates.
  const validated = spawnSync(native, ["--validate-contract", contractFile], {
    encoding: "utf8"
  });
  assert.equal(validated.status, 0, validated.stdout + validated.stderr);

  if (!existsSync(typeFacts)) {
    t.diagnostic(`skipped the consumer assertions: no TypeFacts service at ${typeFacts}`);
    return;
  }
  // What the verified contract is actually worth to a project. The probed row
  // certifies; the converted domain is a demand-sensitive SC9005 exactly where
  // the unknown surface is touched.
  writeFileSync(
    join(directory, "certified.ts"),
    [
      'import { createSignal } from "solid-js";',
      'import { wrapMemo } from "pipeline-fixture";',
      "const [count] = createSignal(0);",
      "function readTracked() { return count(); }",
      "export function exercise() { wrapMemo(readTracked); }",
      ""
    ].join("\n")
  );
  writeFileSync(
    join(directory, "unknown.ts"),
    [
      'import { createSignal } from "solid-js";',
      'import { needsOptions } from "pipeline-fixture";',
      "const [count] = createSignal(0);",
      "function readUnknown() { return count(); }",
      "export function exercise() { needsOptions({ value: 1 }, readUnknown); }",
      ""
    ].join("\n")
  );
  const analyze = file => {
    const project = join(directory, `tsconfig.${file}.json`);
    writeFileSync(
      project,
      JSON.stringify({
        compilerOptions: {
          target: "ES2022",
          module: "ESNext",
          moduleResolution: "bundler",
          strict: true,
          noEmit: true
        },
        files: [`${file}.ts`]
      })
    );
    const child = spawnSync(
      native,
      [
        "--project",
        project,
        "--contract",
        contractFile,
        "--format",
        "json",
        "--typefacts",
        typeFacts,
        "--runtime-target",
        "browser",
        "--runtime-condition",
        "browser",
        "--runtime-condition",
        "import"
      ],
      { encoding: "utf8" }
    );
    return JSON.parse(child.stdout);
  };

  const certified = analyze("certified");
  assert.equal(certified.status, "certified", JSON.stringify(certified.findings));
  assert.equal(
    certified.packageSummaries.find(entry => entry.name === "pipeline-fixture").evidence,
    "verified",
    "the loader accepts the mechanically promoted document as certifying"
  );

  const uncertifiable = analyze("unknown");
  assert.equal(uncertifiable.status, "uncertifiable");
  assert.deepEqual(
    uncertifiable.findings.map(finding => finding.id),
    ["SC9005"]
  );
  assert.match(
    uncertifiable.findings[0].message,
    /leaves callbacks unknown for imported export needsOptions/
  );
});

test(
  "regenerating over a machine-verified contract snapshots it with its sidecars",
  { skip: !canGenerate },
  () => {
    // The `.previous` move exists so a review survives the regeneration that
    // invalidates it, and a verified contract has no review state at all -- so
    // without this it was not snapshotted, and the probe and verify reports for
    // the destroyed bytes were left beside a fresh `inferred` draft claiming to
    // describe it.
    const directory = workspace("solid-checker-verify-regenerate-");
    const output = join(directory, "solid-reactivity.json");
    const sibling = suffix => output.replace(/\.json$/, suffix);
    const document = structuredClone(CONTRACT);
    document.evidence = { kind: "verified" };
    writeFileSync(output, `${JSON.stringify(document, null, 2)}\n`);
    writeFileSync(sibling(".verify.json"), '{"schemaVersion": 1}\n');
    writeFileSync(sibling(".probe.json"), '{"schemaVersion": 1}\n');

    const generated = spawnSync(
      process.execPath,
      [
        cli,
        "contract",
        "generate",
        "--package-root",
        join(root, "fixtures/package-contracts/shorthand-block-scope"),
        "--output",
        output
      ],
      {
        cwd: root,
        encoding: "utf8",
        env: {
          ...process.env,
          SOLID_CHECKER_NATIVE_BIN: native,
          SOLID_TYPEFACTS_BIN: typeFacts
        }
      }
    );
    assert.equal(generated.status, 0, generated.stdout + generated.stderr);
    assert.match(generated.stdout, /a verification is reproduced rather than transferred/);
    assert.match(generated.stdout, /contract probe .* --write && solid-checker contract verify/);
    assert.doesNotMatch(
      generated.stdout,
      /--transfer-from/,
      "there is no review to carry forward, so offering the transfer command would be a lie"
    );

    assert.equal(JSON.parse(readFileSync(sibling(".previous.json"), "utf8")).evidence.kind, "verified");
    assert.equal(existsSync(sibling(".previous.verify.json")), true);
    assert.equal(existsSync(sibling(".previous.probe.json")), true);
    assert.equal(existsSync(sibling(".verify.json")), false, "the fresh draft carries no verification");
    assert.equal(existsSync(sibling(".probe.json")), false);
    assert.equal(JSON.parse(readFileSync(output, "utf8")).evidence.kind, "inferred");
  }
);
