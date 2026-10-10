// Pins that a package generated from `node_modules`, in one module per source
// file with a `.d.ts` beside each, is described exactly as the same code
// bundled into one file (ADR 0132).
//
// Every published package is generated where it is installed, and two
// mechanics made an unbundled one lose every domain on every export:
//
//  1. The resolver reports a relative specifier as `NodeModules` whenever the
//     path it lands on lies under `node_modules`. A relative import between
//     two modules of the package itself therefore read as an import of an
//     external Solid-using package with no accepted contract. That obligation
//     sits at the import declaration, outside every function, and attribution
//     marked every export with it.
//  2. The entry file's `./module.js` import resolves to the sibling
//     `module.d.ts`, so each export's attribution identity was the
//     declaration's. An obligation inside the runtime function in `module.js`
//     then joined to no export, and the ladder fell through to marking every
//     export. The resolution record's exact runtime binding now joins the two.
//
// `@tanstack/solid-router@2.0.0-rc.8` is the measured case: 97 root exports,
// all published `{"call":{}}`.

import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { resolvePackageArtifacts } from "../packages/cli/scripts/artifact-resolution.mjs";
import { mergeProposalDependencies } from "../packages/cli/scripts/certify-contract.mjs";
import { generatePackageContract } from "../packages/cli/scripts/generate-package-contract.mjs";

const root = resolve(import.meta.dirname, "..");
const native = process.env.SOLID_CHECKER_NATIVE_BIN ?? join(root, "rust/target/debug/solid-checker-rust");
const typeFacts = process.env.SOLID_TYPEFACTS_BIN ?? join(root, "bin/solid-typefacts");

if (!existsSync(native) || !existsSync(typeFacts)) {
  throw new Error(
    "the installed-package attribution pin needs fresh native and Type Facts binaries; " +
      "set SOLID_CHECKER_NATIVE_BIN and SOLID_TYPEFACTS_BIN"
  );
}

function write(path, contents) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, contents);
}

// `peerDependencies` names Solid so the package counts as a Solid-using one,
// which is what put its own relative imports on the missing-contract list.
function manifest(name, dependencies) {
  return `${JSON.stringify({
    name,
    version: "1.0.0",
    type: "module",
    exports: { ".": { types: "./dist/index.d.ts", default: "./dist/index.js" } },
    peerDependencies: { "solid-js": "2.0.0-rc.9" },
    ...(dependencies ? { dependencies } : {})
  })}\n`;
}

function summaryOf(document, exportName) {
  const artifactCase = document.entrypoints?.["."]?.cases?.[0];
  const summary = document.summaries?.[artifactCase?.exports?.[exportName]];
  assert.ok(summary, `${exportName} must keep an exact export identity`);
  return summary;
}

function closure(document, exportName) {
  const summary = summaryOf(document, exportName);
  return {
    closed: [...(summary.call?.closed ?? [])].sort(),
    proposed: [...(summary.call?.proposedClosures ?? [])].sort()
  };
}

function invokedFirstArgument(document, exportName) {
  const summary = summaryOf(document, exportName);
  const operations = new Map(
    (summary.call?.operations ?? []).map(operation => [operation.id, operation])
  );
  return (summary.call?.callbacks ?? [])
    .filter(callback => callback.from?.arg === 0 && callback.from?.path?.length === 0)
    .map(callback => operations.get(callback.operation))
    .map(operation => `${operation?.kind}:${operation?.at?.schedule}:${operation?.tracking}`)
    .sort();
}

const MAKE = "function make(handler) {\n  handler();\n}\n";
const USE_OPAQUE =
  "function useOpaque(node) {\n  return opaque(node) ?? fallbackValue;\n}\n";

describe("an installed package is described per export, whatever its module layout", () => {
  let directory;
  const documents = {};

  beforeAll(async () => {
    directory = realpathSync(mkdtempSync(join(tmpdir(), "solid-checker-installed-attribution-")));
    const modules = join(directory, "node_modules");

    // `opaque`'s value comes from an unresolved global, so its contract closes
    // nothing and every use of it is an open-claims obligation. `clean` is
    // there so the dependency's document closes something and can stand in
    // the graph at all (ADR 0129).
    write(join(modules, "depkg/package.json"), manifest("depkg"));
    write(
      join(modules, "depkg/dist/index.js"),
      "export function clean(handler) {\n  handler();\n}\n" +
        "export const opaque = globalThis.hostFactory();\n"
    );
    write(
      join(modules, "depkg/dist/index.d.ts"),
      "export declare function clean(handler: () => void): void;\n" +
        "export declare const opaque: (node: unknown) => unknown;\n"
    );

    // The control: the same two functions in one file.
    write(join(modules, "bundled/package.json"), manifest("bundled", { depkg: "1.0.0" }));
    write(
      join(modules, "bundled/dist/index.js"),
      'import { opaque } from "depkg";\nconst fallbackValue = 1;\n' +
        `${MAKE}${USE_OPAQUE}export { make, useOpaque };\n`
    );
    write(
      join(modules, "bundled/dist/index.d.ts"),
      "export declare function make(handler: () => void): void;\n" +
        "export declare function useOpaque(node: unknown): unknown;\n"
    );

    // The layout a bundler that keeps modules writes: one runtime module and
    // one declaration file per source file, the entry re-exporting them.
    const unbundled = join(modules, "unbundled");
    write(join(unbundled, "package.json"), manifest("unbundled", { depkg: "1.0.0" }));
    write(
      join(unbundled, "dist/index.js"),
      'import { make } from "./make.js";\nimport { useOpaque } from "./use-opaque.js";\n' +
        "export { make, useOpaque };\n"
    );
    write(
      join(unbundled, "dist/index.d.ts"),
      "export { make } from './make.js';\nexport { useOpaque } from './use-opaque.js';\n"
    );
    write(join(unbundled, "dist/make.js"), `${MAKE}export { make };\n`);
    write(
      join(unbundled, "dist/make.d.ts"),
      "export declare function make(handler: () => void): void;\n"
    );
    // A module with a bare import *and* a relative one: the file whose
    // relative edge used to read as a missing package contract.
    write(
      join(unbundled, "dist/use-opaque.js"),
      'import { opaque } from "depkg";\nimport { fallbackValue } from "./fallback.js";\n' +
        `${USE_OPAQUE}export { useOpaque };\n`
    );
    write(
      join(unbundled, "dist/use-opaque.d.ts"),
      "export declare function useOpaque(node: unknown): unknown;\n"
    );
    write(join(unbundled, "dist/fallback.js"), "const fallbackValue = 1;\nexport { fallbackValue };\n");
    write(join(unbundled, "dist/fallback.d.ts"), "export declare const fallbackValue: number;\n");

    // The layout control: `make` in the same sibling module, with nothing
    // else in the package. What the layout itself costs `make` is measured
    // here, so the comparison below isolates what the open dependency next
    // door costs it.
    const alone = join(modules, "alone");
    write(join(alone, "package.json"), manifest("alone"));
    write(join(alone, "dist/index.js"), 'import { make } from "./make.js";\nexport { make };\n');
    write(join(alone, "dist/index.d.ts"), "export { make } from './make.js';\n");
    write(join(alone, "dist/make.js"), `${MAKE}export { make };\n`);
    write(join(alone, "dist/make.d.ts"), "export declare function make(handler: () => void): void;\n");
    const aloneOutput = join(directory, "alone.json");
    await generatePackageContract(
      [
        "--package-root", alone,
        "--output", aloneOutput,
        "--integrity", "sha512-alone",
        "--entrypoint", ".",
        "--certification-importer", join(directory, "app.ts")
      ],
      { quiet: true }
    );
    documents.alone = JSON.parse(readFileSync(aloneOutput, "utf8"));

    const dependencyOutput = join(directory, "depkg.json");
    const generated = await generatePackageContract(
      [
        "--package-root", join(modules, "depkg"),
        "--output", dependencyOutput,
        "--integrity", "sha512-dependency",
        "--entrypoint", ".",
        "--certification-importer", join(modules, "bundled/dist/index.js")
      ],
      { quiet: true }
    );
    const plan = JSON.parse(readFileSync(generated.plan, "utf8"));
    const artifactCases = new Set();
    (function visit(value) {
      if (Array.isArray(value)) for (const child of value) visit(child);
      else if (value && typeof value === "object") {
        if (typeof value.artifactCase === "string") artifactCases.add(value.artifactCase);
        for (const child of Object.values(value)) visit(child);
      }
    })(plan);
    assert.equal(artifactCases.size, 1, "the dependency must resolve one exact artifact case");

    const importers = {
      bundled: [join(modules, "bundled/dist/index.js")],
      unbundled: [join(unbundled, "dist/use-opaque.js")]
    };
    for (const [name, [importer]] of Object.entries(importers)) {
      const merged = mergeProposalDependencies(
        [
          {
            viaSpecifier: "depkg",
            node: { packageName: "depkg", importer },
            planning: {
              proposal: dependencyOutput,
              resolution: resolvePackageArtifacts({
                importer,
                specifier: "depkg",
                conditions: [],
                integrity: "sha512-dependency"
              })
            },
            demandPlan: {
              selectedArtifactCase: [...artifactCases][0],
              candidateSemanticDigest: plan.semanticDigest
            },
            reexportImporters: []
          }
        ],
        join(directory, `${name}-graph`)
      );
      const output = join(directory, `${name}.json`);
      await generatePackageContract(
        [
          "--package-root", join(modules, name),
          "--output", output,
          "--integrity", `sha512-${name}`,
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
      documents[name] = JSON.parse(readFileSync(output, "utf8"));
    }
  }, 300_000);

  afterAll(() => {
    if (directory) rmSync(directory, { recursive: true, force: true });
  });

  test("the control describes the export the open dependency does not reach", () => {
    // Non-vacuity for everything below: `make` states a claim in the bundled
    // layout, so agreeing with it is a claim, not two silences.
    expect(invokedFirstArgument(documents.bundled, "make")).toEqual([
      "invoke:same-stack:ambient-at-execution"
    ]);
    expect(closure(documents.bundled, "make").proposed).not.toEqual([]);
  });

  test("the open dependency next door costs the unbundled export nothing", () => {
    // Falsifier for both mechanics: either one alone marks every export, and
    // `make` then publishes `{"call":{}}` -- no operation, nothing closed.
    expect(invokedFirstArgument(documents.unbundled, "make")).toEqual(
      invokedFirstArgument(documents.bundled, "make")
    );
    const layout = closure(documents.alone, "make");
    expect(layout.closed).toContain("callbacks");
    expect(closure(documents.unbundled, "make")).toEqual(layout);
  });

  test("the export that uses the open dependency stays open", () => {
    // The narrowing moved the obligation, it did not drop it.
    expect(closure(documents.bundled, "useOpaque").closed).not.toContain("callbacks");
    expect(closure(documents.unbundled, "useOpaque").closed).not.toContain("callbacks");
    expect(invokedFirstArgument(documents.unbundled, "useOpaque")).toEqual([]);
  });
});
