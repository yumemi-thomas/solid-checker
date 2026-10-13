// Pins ADR 0156 on the published rc.9 bytes: `@solidjs/web@2.0.0-rc.9`'s
// server build re-exports `ssrScope as scope` from `solid-js/internal`, whose
// `[import,node]` graph node ADR 0129 prunes (it proposes nothing), while its
// typings declare `scope` themselves. The dependent withholds `scope` instead
// of refusing, and binds everything else. Resolution only: the certifier's
// replay is pinned by the certification tests in `contract_certification.rs`
// (`a_runtime_forward_of_a_pruned_export_is_withheld_in_both_shapes` and its
// neighbours).
//
// Resolved against the tsc oracle's verified install of the audited triple
// (`bun scripts/tsc-oracle.mjs provision --dialect v2`).

import { join } from "node:path";
import { describe, expect, test } from "vitest";

import { resolvePackageArtifacts } from "../packages/cli/scripts/artifact-resolution.mjs";
import {
  prunedDependencyRecords,
  withheldExportsOf
} from "../packages/cli/scripts/certify-contract.mjs";
import { oracleProject } from "./tsc-oracle.mjs";

const modules = join(oracleProject("v2").root, "node_modules");
const conditions = ["import", "node"];
const digest = `sha256:${"1".repeat(64)}`;

const resolve = (specifier, packageRoot, importer, acceptedDependencies = {}, prunedDependencies = {}) =>
  resolvePackageArtifacts({
    importer,
    specifier,
    packageRoot,
    conditions,
    integrity: "sha512-audited",
    acceptedDependencies,
    prunedDependencies
  });

const record = (packageName, resolution) => ({
  packageName,
  artifactCase: "artifact-case:test",
  acceptedContractDigest: digest,
  exports: resolution.exports,
  ...withheldExportsOf(resolution)
});

describe("@solidjs/web@2.0.0-rc.9 under node forwards a pruned node's export", () => {
  const webImporter = join(modules, "@solidjs/web/dist/server.js");
  const signals = resolve(
    "@solidjs/signals",
    join(modules, "@solidjs/signals"),
    join(modules, "solid-js/dist/server.js")
  );
  const solid = resolve("solid-js", join(modules, "solid-js"), webImporter, {
    "@solidjs/signals": record("@solidjs/signals", signals)
  });
  const internal = resolve("solid-js/internal", join(modules, "solid-js"), webImporter, {
    "@solidjs/signals": record("@solidjs/signals", signals),
    "solid-js": record("solid-js", solid)
  });
  // The orchestrator's record for the pruned `solid-js/internal` node, exactly
  // as `prunedDependencyRecords` builds it from that node's resolution.
  const pruned = prunedDependencyRecords([
    {
      viaSpecifier: "solid-js/internal",
      state: { node: { packageName: "solid-js" }, planning: { resolution: internal } }
    }
  ]);
  const accepted = {
    "solid-js": record("solid-js", solid),
    "@solidjs/signals": record("@solidjs/signals", signals)
  };
  const web = prunedDependencies =>
    resolve(
      "@solidjs/web",
      join(modules, "@solidjs/web"),
      join(modules, "consumer.mjs"),
      accepted,
      prunedDependencies
    );

  test("the pruned node's record lists its own exact exports only", () => {
    expect(pruned["solid-js/internal"].packageName).toBe("solid-js");
    expect(pruned["solid-js/internal"].exports).toContain("ssrScope");
    // Forwarded from @solidjs/signals, so not the pruned package's own.
    expect(pruned["solid-js/internal"].exports).not.toContain("sourceGet");
  });

  test("scope is withheld -- its runtime forwards the pruned node, its declaration is web's own -- and the rest binds", () => {
    // `types/client.d.ts` declares `export declare function scope(...)`; the
    // server build forwards `solid-js/internal`'s `ssrScope` (ADR 0156).
    const resolution = web(pruned);
    expect(resolution.runtimeWithheldExports).toEqual(["scope"]);
    expect(resolution.forwardedForeignExports).toEqual(["getOwner", "mergeProps", "untrack"]);
    expect(resolution.exports).not.toHaveProperty("scope");
    expect(resolution.exports.createComponent.runtime).toEqual(solid.exports.createComponent.runtime);
    expect(Object.keys(resolution.exports).length).toBeGreaterThan(100);
    // Transitive under ADR 0154: the dependent's own record withholds it.
    expect(withheldExportsOf(resolution).withheldExports).toContain("scope");
  });

  test("a node that is not pruned -- refused, or no record -- still refuses the dependent", () => {
    expect(() => web({})).toThrow(
      "accepted dependency solid-js/internal has no exact runtime binding for export ssrScope"
    );
  });

  test("a record that does not list the forwarded name still refuses", () => {
    const forged = {
      "solid-js/internal": {
        ...pruned["solid-js/internal"],
        exports: pruned["solid-js/internal"].exports.filter(name => name !== "ssrScope")
      }
    };
    expect(() => web(forged)).toThrow(
      "accepted dependency solid-js/internal has no exact runtime binding for export ssrScope"
    );
  });
});
