// Pins ADR 0150 end to end: a runtime definition of the package's own whose
// declaration is another package's costs only that export.
//
// `solid-js@2.0.0-rc.9`'s server build defines `action` (and 25 more names)
// itself while its typings re-export `@solidjs/signals`'. The resolver bound
// the runtime locally and the declaration to the dependency, the emitter
// published the name, and `bind_exports` refused the whole `[import,node]`
// graph node -- 60 `@solid-primitives` packages fell off the graph lane.
//
// The contract corpus pins the standalone half of
// `fixtures/package-contracts/foreign-declaration-reexport` (it refuses: no
// exact declaration binding exists). This test generates the same bytes the
// way the graph lane does, against a private proposal dependency.

import assert from "node:assert/strict";
import { cpSync, existsSync, mkdtempSync, readFileSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { resolvePackageArtifacts } from "../packages/cli/scripts/artifact-resolution.mjs";
import { mergeProposalDependencies } from "../packages/cli/scripts/certify-contract.mjs";
import { generatePackageContract } from "../packages/cli/scripts/generate-package-contract.mjs";

const root = resolve(import.meta.dirname, "..");
const native = process.env.SOLID_CHECKER_NATIVE_BIN ?? join(root, "rust/target/debug/solid-checker-rust");
const typeFacts = process.env.SOLID_TYPEFACTS_BIN ?? join(root, "bin/solid-typefacts");
const fixture = join(root, "fixtures/package-contracts/foreign-declaration-reexport");

if (!existsSync(native) || !existsSync(typeFacts)) {
  throw new Error(
    "the foreign declaration pin needs fresh native and Type Facts binaries; " +
      "set SOLID_CHECKER_NATIVE_BIN and SOLID_TYPEFACTS_BIN"
  );
}

function artifactCaseOf(plan) {
  const cases = new Set();
  (function visit(value) {
    if (Array.isArray(value)) for (const child of value) visit(child);
    else if (value && typeof value === "object") {
      if (typeof value.artifactCase === "string") cases.add(value.artifactCase);
      for (const child of Object.values(value)) visit(child);
    }
  })(plan);
  assert.equal(cases.size, 1, "the dependency must resolve one exact artifact case");
  return [...cases][0];
}

describe("a local definition declared by another package leaves the surface alone", () => {
  let directory;
  let document;
  let generated;

  beforeAll(async () => {
    directory = realpathSync(mkdtempSync(join(tmpdir(), "solid-checker-foreign-declaration-")));
    const packageRoot = join(directory, "node_modules/foreign-declaration-package");
    cpSync(fixture, packageRoot, {
      recursive: true,
      filter: source => !/expected-refusals?\.(json|txt)$|README\.md$/.test(source)
    });
    // Hoist the dependency the way an installer does, beside its dependent.
    cpSync(
      join(packageRoot, "node_modules/declaring-package"),
      join(directory, "node_modules/declaring-package"),
      { recursive: true }
    );
    rmSync(join(packageRoot, "node_modules"), { recursive: true, force: true });

    const importer = join(packageRoot, "dist/index.js");
    const dependencyOutput = join(directory, "declaring.json");
    const dependency = await generatePackageContract(
      [
        "--package-root", join(directory, "node_modules/declaring-package"),
        "--output", dependencyOutput,
        "--integrity", "sha512-declaring",
        "--entrypoint", ".",
        "--certification-importer", importer
      ],
      { quiet: true }
    );
    const plan = JSON.parse(readFileSync(dependency.plan, "utf8"));
    const merged = mergeProposalDependencies(
      [
        {
          viaSpecifier: "declaring-package",
          node: { packageName: "declaring-package", importer },
          planning: {
            proposal: dependencyOutput,
            resolution: resolvePackageArtifacts({
              importer,
              specifier: "declaring-package",
              conditions: [],
              integrity: "sha512-declaring"
            })
          },
          demandPlan: {
            selectedArtifactCase: artifactCaseOf(plan),
            candidateSemanticDigest: plan.semanticDigest
          },
          reexportImporters: [importer]
        }
      ],
      join(directory, "graph")
    );
    const output = join(directory, "foreign.json");
    generated = await generatePackageContract(
      [
        "--package-root", packageRoot,
        "--output", output,
        "--integrity", "sha512-foreign",
        "--entrypoint", ".",
        "--certification-importer", join(directory, "app.ts")
      ],
      {
        quiet: true,
        proposalDependencies: merged.proposalDependencies,
        proposalDependencyCatalog: merged.catalog,
        privateGraphPreparation: true
      }
    );
    document = JSON.parse(readFileSync(output, "utf8"));
  }, 300_000);

  afterAll(() => {
    if (directory) rmSync(directory, { recursive: true, force: true });
  });

  test("the contract publishes every export except the foreign one", () => {
    const artifactCase = document.entrypoints?.["."]?.cases?.[0];
    expect(Object.keys(artifactCase?.exports ?? {}).sort()).toEqual(["own", "together"]);
  });

  test("the resolution names the withheld export for the certifier to replay", () => {
    const resolutions = generated.certificationInputs.map(input => input.resolution);
    expect(resolutions).toHaveLength(1);
    expect(resolutions[0].foreignDeclarationExports).toEqual(["action"]);
    expect(Object.keys(resolutions[0].exports).sort()).toEqual(["own", "together"]);
    expect(resolutions[0].unboundDeclarationExports).toBeUndefined();
  });
});
