// Pins the published-graph lane's per-node audit records.
//
// The graph lane regenerates every node -- its root included -- in private
// certification scratch and removes that scratch when the transaction ends.
// Until these records existed, a graph-lane answer that left a domain open
// said nothing about why: the certification metric could not tell a node
// that never proposed a domain from one whose generator declined it
// (docs/package-contract-v2/phase22/2026-09-28-certification-metric-baseline.md,
// wall 1). The record is read from the exact sidecars a graph node's
// generation writes (`privateGraphPreparation`), so this test generates one
// rather than hand-writing their shape.

import assert from "node:assert/strict";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterAll, beforeAll, test } from "vitest";

import { generatePackageContract } from "../packages/cli/scripts/generate-package-contract.mjs";
import {
  boundedRecords,
  graphNodeAuditRecord,
  graphNodeCasesFromNativeOutput,
  graphNodeRecordSet,
  graphNodesAuditField
} from "../packages/cli/scripts/graph-node-records.mjs";

const root = resolve(import.meta.dirname, "..");
const native = process.env.SOLID_CHECKER_NATIVE_BIN ?? join(root, "rust/target/debug/solid-checker-rust");
const typeFacts = process.env.SOLID_TYPEFACTS_BIN ?? join(root, "bin/solid-typefacts");

if (!existsSync(native) || !existsSync(typeFacts)) {
  throw new Error(
    "the graph-node record pin needs fresh native and Type Facts binaries; " +
      "set SOLID_CHECKER_NATIVE_BIN and SOLID_TYPEFACTS_BIN"
  );
}

// A fixture whose proposal declines closures of four kinds; its
// `expected-refusals.json` is the contract corpus's pin of the same census.
const fixture = join(root, "fixtures/package-contracts/creates-decline-records");

let directory;
let generated;
let savedNative;
let savedTypeFacts;

beforeAll(async () => {
  savedNative = process.env.SOLID_CHECKER_NATIVE_BIN;
  savedTypeFacts = process.env.SOLID_TYPEFACTS_BIN;
  process.env.SOLID_CHECKER_NATIVE_BIN = native;
  process.env.SOLID_TYPEFACTS_BIN = typeFacts;
  directory = mkdtempSync(join(tmpdir(), "solid-checker-graph-node-records-"));
  // Exactly the generation a graph node runs in `preparePublishedGraphCases`.
  generated = await generatePackageContract(
    [
      "--package-root", fixture,
      "--output", join(directory, "solid-reactivity.json"),
      "--integrity", "sha512-graph-node-records",
      "--entrypoint", "."
    ],
    { quiet: true, privateGraphPreparation: true, exactConditions: [] }
  );
}, 180_000);

afterAll(() => {
  if (directory) rmSync(directory, { recursive: true, force: true });
  if (savedNative === undefined) delete process.env.SOLID_CHECKER_NATIVE_BIN;
  else process.env.SOLID_CHECKER_NATIVE_BIN = savedNative;
  if (savedTypeFacts === undefined) delete process.env.SOLID_TYPEFACTS_BIN;
  else process.env.SOLID_TYPEFACTS_BIN = savedTypeFacts;
});

const node = {
  packageName: "creates-decline-records-package",
  packageVersion: "1.0.0",
  entrypoint: ".",
  conditions: ["import"]
};

test("a graph node's closure declines reach its audit record with kind, callee and location", () => {
  const census = JSON.parse(readFileSync(`${generated.output}.refusals.json`, "utf8"));
  assert.ok(census.declinedClosures.length > 0, "the fixture must decline at least one closure");
  const record = graphNodeAuditRecord({
    node,
    isRoot: true,
    artifactCase: "artifact-case:fixture",
    output: generated.output,
    plan: generated.plan
  });
  assert.equal(record.package, "creates-decline-records-package");
  assert.equal(record.root, true);
  assert.equal(record.artifactCase, "artifact-case:fixture");
  // Every decline the generator wrote, one for one, with the fields the
  // metric classifies by.
  assert.deepEqual(
    record.declinedClosures.map(entry => [entry.export, entry.domain, entry.kind]),
    census.declinedClosures.map(entry => [entry.export, entry.domain, entry.kind])
  );
  const dialectSilent = record.declinedClosures.find(entry => entry.kind === "dialect-silent");
  assert.deepEqual(
    [dialectSilent.export, dialectSilent.domain, dialectSilent.package, dialectSilent.callee],
    ["dialectSilent", "creates", "solid-js", "createEffect"]
  );
  assert.match(dialectSilent.location, /^<package-root>\/index\.js:\d+:\d+$/);
  // And the planner's side: what it could not resolve, by export and domain.
  const plan = JSON.parse(readFileSync(generated.plan, "utf8"));
  assert.equal(record.unresolvedClaims.length, new Set(plan.unresolvedClaims.map(claim => JSON.stringify(claim.subject))).size);
  assert.ok(record.unresolvedClaims.every(claim => typeof claim.export === "string" && typeof claim.path?.kind === "string"));
  assert.ok(record.unresolvedClaims.some(claim => claim.path.kind === "call" && claim.path.domain === "creates"));
  assert.equal(record.truncated, undefined);
});

test("records are deduplicated and capped with the remainder counted, never dropped silently", () => {
  const record = graphNodeAuditRecord({
    node,
    output: generated.output,
    plan: generated.plan,
    limit: 1
  });
  const census = JSON.parse(readFileSync(`${generated.output}.refusals.json`, "utf8"));
  assert.equal(record.declinedClosures.length, 1);
  assert.equal(record.truncated.declinedClosures, census.declinedClosures.length - 1);
  assert.deepEqual(boundedRecords([{ a: 1 }, { a: 1 }, { a: 2 }], 5), { items: [{ a: 1 }, { a: 2 }], dropped: 0 });
  assert.deepEqual(boundedRecords([{ a: 1 }, { a: 2 }, { a: 3 }], 2), { items: [{ a: 1 }, { a: 2 }], dropped: 1 });

  // Importer variants of one artifact generate once and record identically.
  const variant = graphNodeAuditRecord({ node, output: generated.output, plan: generated.plan });
  const set = graphNodeRecordSet([variant, JSON.parse(JSON.stringify(variant))]);
  assert.equal(set.nodes.length, 1);
  assert.equal(set.truncated, 0);
  assert.equal(graphNodeRecordSet([variant, { ...variant, entrypoint: "./other" }], 1).truncated, 1);
});

test("a node that refused before generating records its refusal and nothing it did not see", () => {
  const record = graphNodeAuditRecord({
    node,
    output: join(directory, "never-generated.json"),
    refusal: { stage: "graph-acquisition", reason: "registry refused" }
  });
  assert.deepEqual(record.refusal, { stage: "graph-acquisition", reason: "registry refused" });
  assert.deepEqual(
    [record.declinedClosures, record.unresolvedClaims, record.closureCandidates, record.refusals],
    [[], [], [], []]
  );
});

test("node digests join their artifact case from the per-node recipe-address lines", () => {
  const nodeIdentity = { package: "p", version: "1.0.0", digest: "sha256:node" };
  const stdout = [
    `solid-checker:recipe-addresses=${JSON.stringify({ artifactCase: "artifact-case:a", addresses: [], node: nodeIdentity })}`,
    `solid-checker:recipe-addresses=${JSON.stringify({ artifactCase: "artifact-case:a", addresses: [{ semanticClaimId: "c", recipeAddress: "r" }], node: nodeIdentity })}`,
    // The value-only lane's line names no node, and joins nothing.
    `solid-checker:recipe-addresses=${JSON.stringify({ artifactCase: "artifact-case:b", addresses: [] })}`,
    "solid-checker:recipe-addresses={malformed",
    "unrelated output"
  ].join("\n");
  assert.deepEqual(graphNodeCasesFromNativeOutput(stdout), [
    { digest: "sha256:node", package: "p", version: "1.0.0", artifactCase: "artifact-case:a" }
  ]);
});

test("the audit's graphNodes field carries the node's decline, and is absent off the graph lane", () => {
  const record = graphNodeAuditRecord({
    node,
    isRoot: true,
    artifactCase: "artifact-case:a",
    output: generated.output,
    plan: generated.plan
  });
  const cases = [{ digest: "sha256:node", package: node.packageName, version: "1.0.0", artifactCase: "artifact-case:a" }];
  const field = graphNodesAuditField(graphNodeRecordSet([record]), cases);
  // Exactly what `certify-contract.mjs` writes under `graphNodes`, as JSON.
  const audit = JSON.parse(JSON.stringify({ graphNodes: field }));
  const [written] = audit.graphNodes.records;
  assert.ok(
    written.declinedClosures.some(entry => entry.kind === "dialect-silent" && entry.callee === "createEffect"),
    "the node's own dialect-silent decline is in the audit"
  );
  assert.deepEqual(audit.graphNodes.cases, cases);
  assert.equal("truncated" in audit.graphNodes, false);
  assert.equal(graphNodesAuditField(null, cases), null);
  assert.equal(graphNodesAuditField(graphNodeRecordSet([record, { ...record, entrypoint: "./b" }], 1), null).truncated, 1);
});
