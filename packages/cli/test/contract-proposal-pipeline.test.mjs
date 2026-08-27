import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, describe, expect, test, vi } from "vitest";

import {
  contractProposalStages,
  runContractProposalPipeline,
  standaloneProposalAcquisition
} from "../scripts/contract-proposal-pipeline.mjs";

const roots = [];

afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

function rustProduct(stage) {
  return Object.freeze({ authority: "rust", stage });
}

describe("replacement contract proposal pipeline", () => {
  test("keeps all seven stages explicit and ordered", async () => {
    const calls = [];
    const stage = (name, result) =>
      vi.fn(async input => {
        calls.push(name);
        return typeof result === "function" ? result(input) : result;
      });
    const acquisition = {
      discoverPackage: stage("packageDiscovery", { package: "example" }),
      resolveArtifacts: stage("artifactResolution", { artifact: "exact" })
    };
    const rust = {
      analyze: stage("semanticAnalysis", rustProduct("analysis")),
      constructProposal: stage("proposalConstruction", rustProduct("proposal")),
      planProofs: stage("proofPlanning", rustProduct("proofs")),
      planProbes: stage("probePlanning", rustProduct("probes"))
    };
    const output = {
      emit: stage("emission", input => ({
        proposal: input.proposalConstruction,
        proofs: input.proofPlanning,
        probes: input.probePlanning
      }))
    };

    const result = await runContractProposalPipeline({
      request: { specifier: "example" },
      acquisition,
      rust,
      output
    });

    expect(calls).toEqual(contractProposalStages);
    expect(result).toEqual({
      proposal: rustProduct("proposal"),
      proofs: rustProduct("proofs"),
      probes: rustProduct("probes")
    });
  });

  test("passes exact acquisition records through without semantic projection", async () => {
    const packageDiscovery = Object.freeze({ manifest: "sha256:manifest" });
    const artifactResolution = Object.freeze({ closure: "sha256:closure" });
    const analysis = rustProduct("analysis");
    const proposal = rustProduct("proposal");
    const proofs = rustProduct("proofs");
    const probes = rustProduct("probes");
    const analyze = vi.fn(async input => {
      expect(input.packageDiscovery).toBe(packageDiscovery);
      expect(input.artifactResolution).toBe(artifactResolution);
      return analysis;
    });
    const constructProposal = vi.fn(async input => {
      expect(input.semanticAnalysis).toBe(analysis);
      return proposal;
    });
    const planProofs = vi.fn(async input => {
      expect(input.proposalConstruction).toBe(proposal);
      return proofs;
    });
    const planProbes = vi.fn(async input => {
      expect(input.proposalConstruction).toBe(proposal);
      expect(input.proofPlanning).toBe(proofs);
      return probes;
    });

    await runContractProposalPipeline({
      request: Object.freeze({ specifier: "example" }),
      acquisition: {
        discoverPackage: async () => packageDiscovery,
        resolveArtifacts: async () => artifactResolution
      },
      rust: { analyze, constructProposal, planProofs, planProbes },
      output: { emit: async input => input }
    });

    expect(analyze).toHaveBeenCalledOnce();
    expect(constructProposal).toHaveBeenCalledOnce();
    expect(planProofs).toHaveBeenCalledOnce();
    expect(planProbes).toHaveBeenCalledOnce();
  });

  test("refuses a semantic stage result without the Rust protocol discriminator", async () => {
    await expect(
      runContractProposalPipeline({
        request: {},
        acquisition: {
          discoverPackage: async () => ({}),
          resolveArtifacts: async () => ({})
        },
        rust: {
          analyze: async () => ({ authority: "javascript" }),
          constructProposal: async () => rustProduct("proposal"),
          planProofs: async () => rustProduct("proofs"),
          planProbes: async () => rustProduct("probes")
        },
        output: { emit: async () => ({}) }
      })
    ).rejects.toThrow("semanticAnalysis must return a Rust-owned product");
  });

  test("wires Phase 7 exact standalone acquisition into the pipeline seam", async () => {
    const root = mkdtempSync(join(tmpdir(), "solid-checker-phase8-"));
    roots.push(root);
    mkdirSync(join(root, "dist"));
    mkdirSync(join(root, "types"));
    writeFileSync(
      join(root, "package.json"),
      `${JSON.stringify({
        name: "phase-eight",
        version: "1.0.0",
        exports: {
          ".": { types: "./types/index.d.ts", import: "./dist/index.js" }
        }
      })}\n`
    );
    writeFileSync(join(root, "dist/index.js"), "export const value = 1;\n");
    writeFileSync(join(root, "types/index.d.ts"), "export declare const value: 1;\n");
    const acquisition = standaloneProposalAcquisition();
    const request = {
      packageRoot: root,
      importer: join(root, "consumer.ts"),
      specifier: "phase-eight",
      integrity: "sha512:exact"
    };
    const discovered = await acquisition.discoverPackage(request);
    const resolved = await acquisition.resolveArtifacts({
      request,
      packageDiscovery: discovered
    });

    expect(discovered.manifest.name).toBe("phase-eight");
    expect(resolved.runtimeTrace.branch).toBe("/exports/./import");
    expect(resolved.declarationTrace.branch).toBe("/exports/./types");
    expect(resolved.exports.value.runtime.exportName).toBe("value");
  });
});
