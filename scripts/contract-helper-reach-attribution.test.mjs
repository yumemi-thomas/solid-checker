// Pins ADR 0135: an obligation at an import binding used only inside private
// helpers belongs to the exports the call graph says reach those helpers --
// and reachability never narrows through a helper whose module publishes it
// to an importer the call graph cannot join to it.
//
// `@tanstack/solid-router@2.0.0-rc.8`'s `ScrollRestoration.js` is the exact
// case (`setupScrollRestoration` used only in the private
// `useScrollRestoration`). Its `Transitioner.js` and `not-found.js` are the
// guard's: each helper is published by its own module and reached from a
// module whose `./x.js` import resolves to `x.d.ts`, so the graph saw no
// caller and attributed four obligations to no export at all.

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
    "the helper reach pin needs fresh native and Type Facts binaries; " +
      "set SOLID_CHECKER_NATIVE_BIN and SOLID_TYPEFACTS_BIN"
  );
}

function write(path, contents) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, contents);
}

function manifest(name, dependencies) {
  return `${JSON.stringify({
    name,
    version: "1.0.0",
    type: "module",
    exports: { ".": { types: "./dist/index.d.ts", default: "./dist/index.js" } },
    ...(dependencies ? { dependencies } : {})
  })}\n`;
}

function closed(document, exportName) {
  const artifactCase = document.entrypoints?.["."]?.cases?.[0];
  const summary = document.summaries?.[artifactCase?.exports?.[exportName]];
  assert.ok(summary, `${exportName} must keep an exact export identity`);
  return [...(summary.call?.closed ?? [])].sort();
}

const LOCAL = "export function local(handler) {\n  handler();\n}\n";
const LOCAL_DECLARATION = "export declare function local(handler: () => void): void;\n";
const CALLER_DECLARATION = "export declare function caller(value: unknown): unknown;\n";

describe("an obligation in a private helper belongs to the exports that reach it", () => {
  let directory;
  const documents = {};

  beforeAll(async () => {
    directory = realpathSync(mkdtempSync(join(tmpdir(), "solid-checker-helper-reach-")));
    const modules = join(directory, "node_modules");

    write(join(modules, "depkg/package.json"), manifest("depkg"));
    write(
      join(modules, "depkg/dist/index.js"),
      "export function clean(handler) {\n  handler();\n}\n" +
        "export const opaque = globalThis.hostFactory();\n"
    );
    write(
      join(modules, "depkg/dist/index.d.ts"),
      "export declare function clean(handler: () => void): void;\n" +
        "export declare const opaque: (value: unknown) => unknown;\n"
    );

    const packages = {
      control: { "dist/index.js": LOCAL, "dist/index.d.ts": LOCAL_DECLARATION },
      // The helper is private to the entry, so the call graph is exact.
      private: {
        "dist/index.js":
          'import { opaque } from "depkg";\n' +
          "function helper(value) {\n  return opaque(value);\n}\n" +
          `export function caller(value) {\n  return helper(value);\n}\n${LOCAL}`,
        "dist/index.d.ts": `${CALLER_DECLARATION}${LOCAL_DECLARATION}`
      },
      // The helper's own module publishes it, and its importer resolves
      // `./helper.js` to `helper.d.ts`: the graph cannot see the call.
      split: {
        "dist/index.js": `import { caller } from "./caller.js";\nexport { caller };\n${LOCAL}`,
        "dist/index.d.ts": `export { caller } from './caller.js';\n${LOCAL_DECLARATION}`,
        "dist/caller.js":
          'import { helper } from "./helper.js";\nfunction caller(value) {\n  return helper(value);\n}\nexport { caller };\n',
        "dist/caller.d.ts": CALLER_DECLARATION,
        "dist/helper.js":
          'import { opaque } from "depkg";\nfunction helper(value) {\n  return opaque(value);\n}\nexport { helper };\n',
        "dist/helper.d.ts": "export declare function helper(value: unknown): unknown;\n"
      }
    };

    const dependencyOutput = join(directory, "depkg.json");
    const generated = await generatePackageContract(
      [
        "--package-root", join(modules, "depkg"),
        "--output", dependencyOutput,
        "--integrity", "sha512-dependency",
        "--entrypoint", ".",
        "--certification-importer", join(directory, "app.ts")
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

    for (const [name, files] of Object.entries(packages)) {
      const packageRoot = join(modules, name);
      write(join(packageRoot, "package.json"), manifest(name, { depkg: "1.0.0" }));
      for (const [file, contents] of Object.entries(files)) write(join(packageRoot, file), contents);
      const options = { quiet: true };
      const importer = join(packageRoot, name === "split" ? "dist/helper.js" : "dist/index.js");
      if (name !== "control") {
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
        Object.assign(options, {
          proposalDependencies: merged.proposalDependencies,
          proposalDependencyCatalog: merged.catalog,
          privateGraphPreparation: true
        });
      }
      const output = join(directory, `${name}.json`);
      await generatePackageContract(
        [
          "--package-root", packageRoot,
          "--output", output,
          "--integrity", `sha512-${name}`,
          "--entrypoint", ".",
          "--certification-importer", join(directory, "app.ts")
        ],
        options
      );
      documents[name] = JSON.parse(readFileSync(output, "utf8"));
    }
  }, 300_000);

  afterAll(() => {
    if (directory) rmSync(directory, { recursive: true, force: true });
  });

  test("an exact private helper opens its callers and nothing else", () => {
    expect(closed(documents.control, "local")).toContain("callbacks");
    expect(closed(documents.private, "local")).toEqual(closed(documents.control, "local"));
    expect(closed(documents.private, "caller")).not.toContain("callbacks");
  });

  test("a helper the graph cannot join to its importer attributes to no fewer exports", () => {
    // Either the obligation reaches `caller` exactly or it marks every export;
    // what it must never do is reach nothing and leave `caller` closed.
    expect(closed(documents.split, "caller")).not.toContain("callbacks");
  });
});
