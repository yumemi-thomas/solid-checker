// Pins ADR 0154 on the published rc.9 bytes: `@solidjs/web@2.0.0-rc.9`'s
// server build re-exports `getOwner`, `untrack` and `merge as mergeProps` from
// `solid-js`, whose `[import,node]` node withholds all three as foreign
// declaration exports (ADR 0150). The dependent's three exports are those
// unavailable exports and leave its surface alone.
//
// Resolved against the tsc oracle's verified install of the audited triple
// (`bun scripts/tsc-oracle.mjs provision --dialect v2`), the same bytes every
// oracle case compiles against. Resolution only: the certifier's replay of the
// same census is pinned by the certification tests
// `a_forward_of_a_withheld_name_costs_only_that_export` and
// `a_forward_of_a_withheld_name_is_replayed_never_trusted`.

import { join } from "node:path";
import { describe, expect, test } from "vitest";

import { resolvePackageArtifacts } from "../packages/cli/scripts/artifact-resolution.mjs";
import { withheldExportsOf } from "../packages/cli/scripts/certify-contract.mjs";
import { oracleProject } from "./tsc-oracle.mjs";

const modules = join(oracleProject("v2").root, "node_modules");
const conditions = ["import", "node"];
const digest = `sha256:${"1".repeat(64)}`;

const resolve = (specifier, packageRoot, importer, acceptedDependencies = {}) =>
  resolvePackageArtifacts({
    importer,
    specifier,
    packageRoot,
    conditions,
    integrity: "sha512-audited",
    acceptedDependencies
  });

// The planned dependency's record, exactly as `mergeProposalDependencies`
// writes it.
const record = (packageName, resolution) => ({
  packageName,
  artifactCase: "artifact-case:test",
  acceptedContractDigest: digest,
  exports: resolution.exports,
  ...withheldExportsOf(resolution)
});

describe("@solidjs/web@2.0.0-rc.9 under node forwards solid-js's withheld names", () => {
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
  const dependencies = solidRecord => ({
    "solid-js": solidRecord,
    "solid-js/internal": record("solid-js", internal),
    "@solidjs/signals": record("@solidjs/signals", signals)
  });

  test("solid-js withholds the three names under ADR 0150", () => {
    expect(solid.foreignDeclarationExports).toEqual(
      expect.arrayContaining(["getOwner", "merge", "untrack"])
    );
    expect(record("solid-js", solid).withheldExports).toEqual(
      expect.arrayContaining(["getOwner", "merge", "untrack"])
    );
  });

  test("the dependent withholds getOwner, mergeProps and untrack, and binds the rest", () => {
    const web = resolve(
      "@solidjs/web",
      join(modules, "@solidjs/web"),
      join(modules, "consumer.mjs"),
      dependencies(record("solid-js", solid))
    );
    expect(web.forwardedForeignExports).toEqual(["getOwner", "mergeProps", "untrack"]);
    expect(web.exports).not.toHaveProperty("getOwner");
    // A sibling forward from the same dependency still binds exactly.
    expect(web.exports.createComponent.runtime).toEqual(solid.exports.createComponent.runtime);
  });

  test("without the dependency's withheld census the dependent refuses as before", () => {
    const { withheldExports: _withheld, ...stale } = record("solid-js", solid);
    expect(() =>
      resolve(
        "@solidjs/web",
        join(modules, "@solidjs/web"),
        join(modules, "consumer.mjs"),
        dependencies(stale)
      )
    ).toThrow("accepted dependency solid-js has no exact runtime binding for export getOwner");
  });
});
