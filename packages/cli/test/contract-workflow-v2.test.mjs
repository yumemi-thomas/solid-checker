import assert from "node:assert/strict";
import { test } from "vitest";

import { createRuntimeProbeHarness } from "../scripts/contract-probe-harness.mjs";
import { parseProbeArguments } from "../scripts/probe-contract.mjs";
import { parseReviewArguments } from "../scripts/review-contract.mjs";
import { parseVerifyArguments } from "../scripts/verify-contract.mjs";

test("review exposes only temporary-v2 proposal inspection", () => {
  assert.deepEqual(parseReviewArguments(["proposal.json", "--output", "review.json"]), {
    proposal: "proposal.json",
    output: "review.json",
    help: false
  });
  assert.throws(() => parseReviewArguments(["proposal.json", "--promote", "reviewed"]), /unknown/);
});

test("verification requires separate plan, proof, and exact artifact case", () => {
  const parsed = parseVerifyArguments([
    "proposal.json",
    "--plan=plan.json",
    "--proof",
    "proof.json",
    "--artifact-case",
    "case"
  ]);
  assert.equal(parsed.plan, "plan.json");
  assert.equal(parsed.proof, "proof.json");
  assert.equal(parsed.artifactCase, "case");
  assert.throws(() => parseVerifyArguments(["proposal.json"]), /--plan is required/);
});

test("probe parsing has no write or negative-discovery compatibility mode", () => {
  const parsed = parseProbeArguments([
    "proposal.json",
    "--request",
    "request.json",
    "--plan-only"
  ]);
  assert.equal(parsed.request, "request.json");
  assert.equal(parsed.planOnly, true);
  assert.throws(
    () => parseProbeArguments(["proposal.json", "--request", "request.json", "--write"]),
    /unknown/
  );
});

test("worker harness transports sequenced events and bounded drain counts", async () => {
  const harness = createRuntimeProbeHarness({
    drain: [
      { kind: "flush" },
      { kind: "microtasks", maxTurns: 2 },
      { kind: "macrotasks", maxTurns: 1 }
    ]
  });
  let flushed = 0;
  harness.emit({ marker: "first", kind: "call", phase: "enter" });
  harness.emit({ marker: "second", kind: "callback", ordinal: 0 });
  await harness.drain({ flush: () => (flushed += 1) });
  assert.deepEqual(harness.events().map(event => event.sequence), [0, 1]);
  assert.equal(harness.drainedMicrotasks(), 2);
  assert.equal(harness.drainedMacrotasks(), 1);
  assert.equal(flushed, 1);
});
