// Run with Node from any directory. This is an audit experiment using the
// current, unmodified probe worker; it mints no certification authority.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import {
  runProbeSessions,
  sha256Bytes
} from "../../../packages/cli/scripts/contract-probe-driver.mjs";
import {
  PROBE_WORKER_PROTOCOL
} from "../../../packages/cli/scripts/contract-probe-harness.mjs";

const cases = [
  { name: "owned-tracked", before: 1, afterOwned: 2, afterCaller: 2, mayVeto: true },
  { name: "owned-tracked-discarded", before: 1, afterOwned: 2, afterCaller: 2, mayVeto: true },
  { name: "no-read", before: 1, afterOwned: 1, afterCaller: 1, mayVeto: false },
  { name: "caller-accessor", before: 1, afterOwned: 1, afterCaller: 2, mayVeto: false },
  { name: "caller-getter", before: 1, afterOwned: 1, afterCaller: 2, mayVeto: false },
  // Same raw rerun signal as owned-tracked, but the subject only invokes
  // caller-authored code. Source identity alone does not establish authorship.
  { name: "caller-accessor-reading-package-source", before: 1, afterOwned: 2, afterCaller: 2, mayVeto: false },
  { name: "owned-untracked", before: 1, afterOwned: 1, afterCaller: 1, mayVeto: false },
  { name: "owned-untracked-discarded", before: 1, afterOwned: 1, afterCaller: 1, mayVeto: false }
];

export async function runProbeSession(session, harness) {
  const graphObservation = session.observation === "development-source-count";
  const { createRoot, createEffect, createSignal, flush, getObserver, DEV } = graphObservation
    ? await import("../../../rust/target/tsc-oracle/v2/node_modules/@solidjs/signals/dist/dev.js")
    : await import("../../../rust/target/tsc-oracle/v2/node_modules/@solidjs/signals/dist/prod/index.js");
  const { loadSubject } = await import("./2026-09-12-reads-observation-experiment.subject.mjs");
  const subject = await loadSubject(graphObservation ? "development" : "production");
  const scenario = cases.find(item => item.name === session.scenario);
  assert.ok(scenario, "the worker must name a reviewed experiment scenario");

  const [callerSource, changeCaller] = createSignal(0);
  const caller = { get value() { return callerSource(); } };
  const invoke = {
    "owned-tracked": () => subject.readsOwned(),
    "owned-tracked-discarded": () => subject.readsOwnedButDiscardsValue(),
    "no-read": () => subject.noRead(),
    "caller-accessor": () => subject.invokesCaller(() => callerSource()),
    "caller-getter": () => subject.readsCallerMember(caller),
    "caller-accessor-reading-package-source": () => subject.invokesCaller(() => subject.readsOwned()),
    "owned-untracked": () => subject.readsOwnedUntracked(),
    "owned-untracked-discarded": () => subject.readsOwnedUntrackedButDiscardsValue()
  }[scenario.name];

  let computes = 0;
  let initialValue;
  let initialSources = -1;
  let dispose;
  createRoot(cleanup => {
    dispose = cleanup;
    createEffect(() => {
      computes += 1;
      harness.emit({ marker: "call", kind: "call", phase: "enter" });
      const result = invoke();
      if (computes === 1) {
        initialValue = result;
        // getSources and getObserver are published exports, not minified
        // field names. They share the subject's exact runtime module instance.
        if (graphObservation) initialSources = DEV.getSources(getObserver()).length;
      }
      harness.emit({ marker: "call", kind: "call", phase: "exit" });
      return result;
    }, () => {});
  });
  try {
    flush();
    const before = computes;
    if (!graphObservation) {
      subject.changeOwned(1);
      flush();
    }
    const afterOwned = computes;
    if (!graphObservation) {
      changeCaller(1);
      flush();
    }
    const afterCaller = computes;

    assert.equal(before, scenario.before, "initial tracked invocation must complete");
    assert.equal(afterOwned, graphObservation ? 1 : scenario.afterOwned, "package source mutation rerun count");
    assert.equal(afterCaller, graphObservation ? 1 : scenario.afterCaller, "caller source mutation rerun count");
    const expectsTrackedSource = !["no-read", "owned-untracked", "owned-untracked-discarded"].includes(scenario.name);
    if (graphObservation) assert.equal(initialSources, Number(expectsTrackedSource), "published development source-list observation");
    else assert.equal(DEV, undefined, "production runtime must not be silently substituted");

    // This eligibility decision comes from the known fixture bodies. The raw
    // invalidation mechanism cannot make it: the caller-authorship collision
    // below is an explicit negative result, not a runtime ownership oracle.
    const rawObservation = graphObservation ? initialSources > 0 : afterOwned > before;
    if (scenario.mayVeto && rawObservation) {
      harness.emit({ marker: "read-operation", kind: "call", phase: "enter" });
    }
    const explicitValueAfterObservation = invoke();
    harness.emit({
      marker: "experiment-observation",
      kind: "call",
      phase: "enter",
      scenario: scenario.name,
      observation: session.observation,
      before,
      afterOwned,
      afterCaller,
      rawInvalidationDetected: afterOwned > before,
      initialSources,
      initialValue,
      explicitValueAfterObservation
    });
  } finally {
    dispose();
    flush();
  }
}

async function main() {
  const module = fileURLToPath(import.meta.url);
  const subjectUrl = new URL("./2026-09-12-reads-observation-experiment.subject.mjs", import.meta.url);
  const runtimeRoot = new URL("../../../rust/target/tsc-oracle/v2/node_modules/@solidjs/signals/", import.meta.url);
  const manifest = JSON.parse(readFileSync(new URL("package.json", runtimeRoot), "utf8"));
  assert.equal(manifest.version, "2.0.0-rc.3", "do not silently substitute a newer runtime");
  const construction = sha256Bytes(readFileSync(module));
  const environment = { runtime: { protocol: PROBE_WORKER_PROTOCOL, version: process.version } };
  const scenarios = ["production-invalidation", "development-source-count"].flatMap(observation =>
    cases.map(scenario => ({ ...scenario, observation }))
  );
  const plan = {
    format: "solid-checker-runtime-probe-plan",
    schemaVersion: 2,
    sessions: scenarios.map(scenario => ({
      id: `reads-observation:${scenario.observation}:${scenario.name}`,
      scenario: scenario.name,
      observation: scenario.observation,
      module,
      construction,
      mode: { environment },
      policy: { timeoutMillis: 5000 },
      drain: [{ kind: "microtasks", maxTurns: 2 }]
    }))
  };
  const runs = await runProbeSessions(plan, module);
  assert.equal(runs.length, scenarios.length);
  const observations = runs.map((run, index) => {
    const scenario = scenarios[index];
    assert.equal(run.outcome.kind, "completed", `${scenario.name}: ${run.outcome.summary ?? run.outcome.reason ?? "did not complete"}`);
    const events = run.outcome.events;
    const markers = events.filter(event => event.marker === "read-operation");
    assert.equal(markers.length, Number(scenario.mayVeto), `${scenario.name}: exact contradiction marker count`);
    const observation = events.find(event => event.marker === "experiment-observation");
    assert.ok(observation, `${scenario.name}: missing observation`);
    const { marker, kind, phase, sequence, ...measured } = observation;
    return { ...measured, emittedContradiction: markers.length === 1 };
  });
  assert.equal(new Set(runs.map(run => run.isolation.process)).size, scenarios.length, "every scenario must use a fresh worker");
  console.log(JSON.stringify({
    experiment: "owned-read-invalidation-and-development-source-count",
    authority: "audit-only; no Rust verdict, private snapshot, or accepted contract",
    node: process.version,
    runtime: {
      package: manifest.name,
      version: manifest.version,
      entries: ["dist/prod/index.js", "dist/dev.js"].map(entry => ({ entry, entryDigest: sha256Bytes(readFileSync(new URL(entry, runtimeRoot))) }))
    },
    subjectDigest: sha256Bytes(readFileSync(subjectUrl)),
    workerProtocol: PROBE_WORKER_PROTOCOL,
    passed: observations.length,
    observations
  }, null, 2));
}

if (process.argv[1] === fileURLToPath(import.meta.url)) await main();
