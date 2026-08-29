import assert from "node:assert/strict";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import { createRuntimeProbeHarness } from "../scripts/contract-probe-harness.mjs";
import {
  CertificationRefusal,
  certifyContract,
  parseCertifyArguments,
  runContractCertificationPipeline
} from "../scripts/certify-contract.mjs";
import { parseProbeArguments } from "../scripts/probe-contract.mjs";
import { parseReviewArguments } from "../scripts/review-contract.mjs";
import { parseVerifyArguments } from "../scripts/verify-contract.mjs";
import {
  finiteEntrypoints,
  retainIndependentlyMergeableProposals
} from "../scripts/generate-package-contract.mjs";

test("review exposes only stable proposal inspection", () => {
  assert.deepEqual(parseReviewArguments(["proposal.json", "--output", "review.json"]), {
    proposal: "proposal.json",
    output: "review.json",
    help: false
  });
  assert.throws(() => parseReviewArguments(["proposal.json", "--promote", "reviewed"]), /unknown/);
});

test("policy-1 caller-proof verification is retired", () => {
  assert.deepEqual(parseVerifyArguments(["--help"]), { help: true });
  assert.throws(
    () => parseVerifyArguments(["proposal.json"]),
    /proof-file issuance was retired/
  );
});

test("policy-2 certification accepts no caller-authored proof or receipt input", () => {
  assert.equal(
    parseCertifyArguments(["--integrity", "sha512-cGlubmVk"]).integrity,
    "sha512-cGlubmVk"
  );
  assert.throws(() => parseCertifyArguments([]), /--integrity is required/);
  assert.throws(
    () => parseCertifyArguments(["--integrity", "sha512-cGlubmVk", "--proof", "proof.json"]),
    /unknown contract certification argument --proof/
  );
  assert.throws(
    () => parseCertifyArguments(["--integrity", "sha512-cGlubmVk", "--receipt", "receipt.json"]),
    /unknown contract certification argument --receipt/
  );
});

test("certification publishes only after every authority stage succeeds", async () => {
  const stages = [];
  const result = await runContractCertificationPipeline({
    request: { package: "example" },
    acquisition: {
      acquireArtifacts: async () => (stages.push("artifact"), { snapshot: "exact" })
    },
    proposal: {
      generate: async () => (stages.push("proposal"), { authority: "rust" })
    },
    rust: {
      planDemands: async () => (stages.push("demands"), { authority: "rust" }),
      certify: async () => (stages.push("certify"), { authority: "rust" })
    },
    evidence: {
      obtainWitnesses: async () => (stages.push("witnesses"), { live: true })
    },
    issuer: {
      issue: async () => (stages.push("receipt"), { authority: "configured-issuer" })
    },
    publication: {
      commit: async () => (stages.push("publish"), "published")
    }
  });
  assert.equal(result, "published");
  assert.deepEqual(stages, [
    "artifact",
    "proposal",
    "demands",
    "witnesses",
    "certify",
    "receipt",
    "publish"
  ]);
});

test("an intermediate certification refusal cannot reach catalog publication", async () => {
  let published = false;
  await assert.rejects(
    runContractCertificationPipeline({
      request: {},
      acquisition: { acquireArtifacts: async () => ({}) },
      proposal: { generate: async () => ({ authority: "rust" }) },
      rust: {
        planDemands: async () => ({ authority: "rust" }),
        certify: async () => ({ authority: "rust" })
      },
      evidence: {
        obtainWitnesses: async () => {
          throw new CertificationRefusal({
            stage: "witness-acquisition",
            owner: "probe-gate",
            demandId: "sha256:missing",
            family: "probe-consistency",
            reason: "missing live harness binding"
          });
        }
      },
      issuer: { issue: async () => ({ authority: "configured-issuer" }) },
      publication: { commit: async () => (published = true) }
    }),
    /witness-acquisition refused for demand sha256:missing/
  );
  assert.equal(published, false);
});

test("concrete acquisition failure writes only a non-replayable audit", async () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-certify-test-"));
  const catalog = join(root, "accepted-contracts.json");
  const audit = join(root, "audit.json");
  writeFileSync(join(root, "package.json"), '{"name":"example","version":"1.0.0"}\n');
  writeFileSync(catalog, "catalog-sentinel\n");
  const metadata = new TextEncoder().encode(
    JSON.stringify({
      versions: {
        "1.0.0": {
          name: "example",
          version: "1.0.0",
          dist: {
            integrity: "sha512-registry",
            tarball: "https://registry.npmjs.org/example/-/example-1.0.0.tgz"
          }
        }
      }
    })
  );
  const fetch_ = async () => ({
    ok: true,
    status: 200,
    arrayBuffer: async () => metadata.buffer
  });
  try {
    await assert.rejects(
      certifyContract(
        [
          "--package-root",
          root,
          "--integrity",
          "sha512-lockfile",
          "--catalog",
          catalog,
          "--audit-output",
          audit
        ],
        { fetch_ }
      ),
      /registry integrity .* disagrees/
    );
    assert.equal(readFileSync(catalog, "utf8"), "catalog-sentinel\n");
    assert.equal(existsSync(audit), true);
    const transcript = JSON.parse(readFileSync(audit, "utf8"));
    assert.equal(transcript.authoritative, false);
    assert.equal(transcript.replayable, false);
    assert.equal(transcript.status, "refused");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
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

test("finite entrypoint discovery keeps exact rows while refusing wildcard coverage", () => {
  assert.deepEqual(
    finiteEntrypoints(
      { exports: { ".": "./index.js", "./web": "./web.js", "./types/*": "./types/*.d.ts" } },
      []
    ),
    { entrypoints: [".", "./web"], wildcardRefusals: ["./types/*"] }
  );
  assert.throws(
    () => finiteEntrypoints({ exports: { "./*": "./dist/*.js" } }, []),
    /pass each finite --entrypoint/
  );
});

test("finite wildcard entrypoints are enumerated from exact package files", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-wildcard-test-"));
  mkdirSync(join(root, "dist", "components"), { recursive: true });
  writeFileSync(join(root, "dist", "components", "button.js"), "export const Button = 1;\n");
  writeFileSync(join(root, "dist", "components", "menu.js"), "export const Menu = 1;\n");
  try {
    assert.deepEqual(
      finiteEntrypoints(
        {
          exports: {
            ".": "./dist/index.js",
            "./components/*": "./dist/components/*.js",
            "./opaque/*": "./generated/no-star.js"
          }
        },
        [],
        root
      ),
      {
        entrypoints: [".", "./components/button", "./components/menu"],
        wildcardRefusals: ["./opaque/*"]
      }
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("a merge contradiction refuses only its exact artifact candidate", async () => {
  const candidates = ["known-a", "contradictory-b", "known-c"].map(entrypoint => ({
    entrypoint
  }));
  const attempts = [];
  const result = await retainIndependentlyMergeableProposals(
    candidates,
    async (merged, candidate) => {
      attempts.push({ merged: merged?.members ?? [], candidate: candidate.entrypoint });
      if (candidate.entrypoint === "contradictory-b") throw new Error("invalid graph");
      return { members: [...(merged?.members ?? []), candidate.entrypoint] };
    }
  );

  assert.equal(result.acceptedCount, 2);
  assert.deepEqual(result.merged.members, ["known-a", "known-c"]);
  assert.deepEqual(result.rejected.map(item => item.candidate.entrypoint), ["contradictory-b"]);
  assert.deepEqual(attempts[2], { merged: ["known-a"], candidate: "known-c" });
});
