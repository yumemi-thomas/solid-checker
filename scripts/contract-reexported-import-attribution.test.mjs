// Pins ADR 0133: an entry-file import that is only re-exported by the entry's
// own export list belongs to the names that list publishes.
//
// `import { x } from "dep"; export { x };` is the two-statement spelling of
// `export { x } from "dep"`, and bundlers emit it for every cross-package
// re-export. The one-statement spelling already had a rung
// (`reexport-specifier`); the two-statement one fell through to marking every
// export, which is what left 29 of `@tanstack/solid-router@2.0.0-rc.8`'s root
// obligations marking all 97 exports.

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
    "the re-exported import pin needs fresh native and Type Facts binaries; " +
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

function closure(document, exportName) {
  const artifactCase = document.entrypoints?.["."]?.cases?.[0];
  const summary = document.summaries?.[artifactCase?.exports?.[exportName]];
  assert.ok(summary, `${exportName} must keep an exact export identity`);
  return {
    closed: [...(summary.call?.closed ?? [])].sort(),
    proposed: [...(summary.call?.proposedClosures ?? [])].sort()
  };
}

const LOCAL = "export function local(handler) {\n  handler();\n}\n";
const LOCAL_DECLARATION = "export declare function local(handler: () => void): void;\n";
const DEPENDENCY_DECLARATIONS =
  "export declare function clean(handler: () => void): void;\n" +
  "export declare const opaque: (node: unknown) => unknown;\n";

describe("an import the entry only re-exports belongs to the names it publishes", () => {
  let directory;
  const documents = {};

  beforeAll(async () => {
    directory = realpathSync(mkdtempSync(join(tmpdir(), "solid-checker-reexported-import-")));
    const modules = join(directory, "node_modules");

    // `opaque` closes nothing, so any binding of it is an open-claims
    // obligation; `clean` lets the dependency document stand in the graph.
    write(join(modules, "depkg/package.json"), manifest("depkg"));
    write(
      join(modules, "depkg/dist/index.js"),
      "export function clean(handler) {\n  handler();\n}\n" +
        "export const opaque = globalThis.hostFactory();\n"
    );
    write(join(modules, "depkg/dist/index.d.ts"), DEPENDENCY_DECLARATIONS);

    const sources = {
      // The spelling that already had a rung: the control.
      onestep: 'export { clean, opaque } from "depkg";\n',
      // The spelling this rung is for.
      twostep: 'import { clean, opaque } from "depkg";\nexport { clean, opaque };\n',
      // The negative: the binding is also read at module level, which is a
      // use this rung must not account for.
      held:
        'import { clean, opaque } from "depkg";\nconst held = opaque;\n' +
        "export { clean, opaque, held };\n"
    };
    const declarations = {
      onestep: 'export { clean, opaque } from "depkg";\n',
      twostep: 'export { clean, opaque } from "depkg";\n',
      held:
        'export { clean, opaque } from "depkg";\n' +
        "export declare const held: (node: unknown) => unknown;\n"
    };
    for (const name of Object.keys(sources)) {
      write(join(modules, name, "package.json"), manifest(name, { depkg: "1.0.0" }));
      write(join(modules, name, "dist/index.js"), `${sources[name]}${LOCAL}`);
      write(join(modules, name, "dist/index.d.ts"), `${declarations[name]}${LOCAL_DECLARATION}`);
    }

    const dependencyOutput = join(directory, "depkg.json");
    const generated = await generatePackageContract(
      [
        "--package-root", join(modules, "depkg"),
        "--output", dependencyOutput,
        "--integrity", "sha512-dependency",
        "--entrypoint", ".",
        "--certification-importer", join(modules, "onestep/dist/index.js")
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

    for (const name of Object.keys(sources)) {
      const importer = join(modules, name, "dist/index.js");
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
            reexportImporters: [importer]
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

  test("the control keeps its own export's closure beside an open re-export", () => {
    expect(closure(documents.onestep, "local").closed).toContain("callbacks");
  });

  test("the two-statement spelling is described exactly as the one-statement one", () => {
    expect(closure(documents.twostep, "local")).toEqual(closure(documents.onestep, "local"));
    expect(closure(documents.twostep, "opaque")).toEqual({ closed: [], proposed: [] });
  });

  test("a binding with any other use still marks every export", () => {
    // `held` reads `opaque` at module evaluation; nothing proves which export
    // that reaches, so the ladder keeps its widest answer.
    expect(closure(documents.held, "local").closed).not.toContain("callbacks");
  });
});
