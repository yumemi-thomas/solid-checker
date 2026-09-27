// Pins ADR 0137: an importer whose `./m.js` TypeScript binds to a sibling
// `m.d.ts` is joined to the runtime module Node loads, so the call graph sees
// its calls -- and only an exact ESM landing makes that join.
//
// `@tanstack/solid-router@2.0.0-rc.8` ships one `.d.ts` beside each runtime
// module. `Matches.js` imports `Transitioner` through `./Transitioner.js`,
// which the compiler resolves to `Transitioner.d.ts`; the graph saw no caller
// of the implementation, ADR 0135's guard refused to narrow, and every
// obligation in `Transitioner.js` marked every export.

import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { afterAll, beforeAll, describe, expect, test } from "vitest";

import { resolvePackageArtifacts, runtimeModuleResolutions } from "../packages/cli/scripts/artifact-resolution.mjs";
import { mergeProposalDependencies } from "../packages/cli/scripts/certify-contract.mjs";
import { generatePackageContract } from "../packages/cli/scripts/generate-package-contract.mjs";

const root = resolve(import.meta.dirname, "..");
const native = process.env.SOLID_CHECKER_NATIVE_BIN ?? join(root, "rust/target/debug/solid-checker-rust");
const typeFacts = process.env.SOLID_TYPEFACTS_BIN ?? join(root, "bin/solid-typefacts");

if (!existsSync(native) || !existsSync(typeFacts)) {
  throw new Error(
    "the runtime edge pin needs fresh native and Type Facts binaries; " +
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
const HELPER =
  'import { opaque } from "depkg";\nfunction helper(value) {\n  return opaque(value);\n}\nexport { helper };\n';
const HELPER_DECLARATION = "export declare function helper(value: unknown): unknown;\n";

// Every package routes `caller -> helper` across a `.d.ts` split; they differ
// only in how `caller.js` names the helper module.
function split(specifier) {
  return {
    "dist/index.js": `import { caller } from "./caller.js";\nexport { caller };\n${LOCAL}`,
    "dist/index.d.ts": `export { caller } from './caller.js';\n${LOCAL_DECLARATION}`,
    "dist/caller.js":
      `import { helper } from "${specifier}";\nfunction caller(value) {\n  return helper(value);\n}\nexport { caller };\n`,
    "dist/caller.d.ts": CALLER_DECLARATION,
    "dist/helper.js": HELPER,
    "dist/helper.d.ts": HELPER_DECLARATION
  };
}

describe("a .d.ts split is joined to the runtime module Node loads", () => {
  let directory;
  const documents = {};
  const edges = {};

  beforeAll(async () => {
    directory = realpathSync(mkdtempSync(join(tmpdir(), "solid-checker-runtime-edge-")));
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
      // The published shape: an explicit `.js` specifier Node loads as written.
      joined: split("./helper.js"),
      // A bundler-only spelling. Node's ESM loader resolves no extensionless
      // relative specifier, so no runtime landing is exact and the join stays
      // refused.
      extensionless: split("./helper")
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
      edges[name] = runtimeModuleResolutions(
        resolvePackageArtifacts({
          importer: join(directory, "app.ts"),
          specifier: name,
          packageRoot,
          conditions: [],
          integrity: `sha512-${name}`
        })
      ).map(edge => ({
        importer: edge.importer.slice(packageRoot.length + 1),
        specifier: edge.specifier,
        target: edge.target.slice(packageRoot.length + 1)
      }));
      const options = { quiet: true };
      const importer = join(packageRoot, name === "control" ? "dist/index.js" : "dist/helper.js");
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

  test("the generator records exactly the landings Node loads", () => {
    expect(edges.joined).toEqual([
      { importer: "dist/caller.js", specifier: "./helper.js", target: "dist/helper.js" },
      { importer: "dist/index.js", specifier: "./caller.js", target: "dist/caller.js" }
    ]);
    expect(edges.extensionless).toEqual([
      { importer: "dist/index.js", specifier: "./caller.js", target: "dist/caller.js" }
    ]);
  });

  test("a joined split attributes the helper to its exact callers", () => {
    expect(closed(documents.control, "local")).toContain("callbacks");
    expect(closed(documents.joined, "local")).toEqual(closed(documents.control, "local"));
    expect(closed(documents.joined, "caller")).not.toContain("callbacks");
  });

  test("a split with no exact runtime landing still marks every export", () => {
    expect(closed(documents.extensionless, "local")).not.toContain("callbacks");
    expect(closed(documents.extensionless, "caller")).not.toContain("callbacks");
  });
});
