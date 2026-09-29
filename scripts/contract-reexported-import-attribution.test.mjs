// Pins ADR 0133: an entry-file import that is only re-exported by the entry's
// own export list belongs to the names that list publishes.
//
// `import { x } from "dep"; export { x };` is the two-statement spelling of
// `export { x } from "dep"`, and bundlers emit it for every cross-package
// re-export. The one-statement spelling already had a rung
// (`reexport-specifier`); the two-statement one fell through to marking every
// export, which is what left 29 of `@tanstack/solid-router@2.0.0-rc.8`'s root
// obligations marking all 97 exports.
//
// ADR 0169 extends it one module down: the barrel a bundler emits beside the
// entry (`dist/transform.js` of `@solid-primitives/sse`) holds the import, and
// the entry republishes it. The rung answers only when the entry's re-export
// chain to the binding is exact; an ambiguous star, a cycle or an unresolved
// edge still marks every export.

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
        "export { clean, opaque, held };\n",
      // ADR 0169: the entry republishes a sibling barrel's binding.
      sibling: 'import { clean, opaque } from "./transform.js";\nexport { clean, opaque };\n',
      siblingRenamed:
        'import { clean, opaque as renamed } from "./transform.js";\nexport { clean, renamed };\n',
      siblingStar: 'export * from "./transform.js";\n',
      // The negatives: the barrel's namespace is also published (it exposes the
      // binding under a name the chain cannot follow), and a star cycle. (Two
      // stars providing one name never reach the ladder: the generator refuses
      // them earlier, "resolves through multiple star exports".)
      siblingNamespace:
        'import * as t from "./transform.js";\nimport { clean, opaque } from "./transform.js";\nexport { clean, opaque, t };\n',
      siblingCycle: 'export * from "./transform.js";\n'
    };
    const barrel = 'import { clean, opaque } from "depkg";\nexport { clean, opaque };\n';
    const barrelDeclarations = 'export { clean, opaque } from "depkg";\n';
    // Files beside `dist/index.js`, by package.
    const siblings = {
      sibling: { "dist/transform.js": barrel, "dist/transform.d.ts": barrelDeclarations },
      siblingRenamed: { "dist/transform.js": barrel, "dist/transform.d.ts": barrelDeclarations },
      siblingStar: { "dist/transform.js": barrel, "dist/transform.d.ts": barrelDeclarations },
      siblingNamespace: { "dist/transform.js": barrel, "dist/transform.d.ts": barrelDeclarations },
      siblingCycle: {
        "dist/transform.js": `${barrel}export * from "./index.js";\n`,
        "dist/transform.d.ts": `${barrelDeclarations}export * from "./index.js";\n`
      }
    };
    const declarations = {
      onestep: 'export { clean, opaque } from "depkg";\n',
      twostep: 'export { clean, opaque } from "depkg";\n',
      held:
        'export { clean, opaque } from "depkg";\n' +
        "export declare const held: (node: unknown) => unknown;\n",
      sibling: 'export { clean, opaque } from "./transform.js";\n',
      siblingRenamed: 'export { clean, opaque as renamed } from "./transform.js";\n',
      siblingStar: 'export * from "./transform.js";\n',
      siblingNamespace: 'export * as t from "./transform.js";\nexport { clean, opaque } from "./transform.js";\n',
      siblingCycle: 'export * from "./transform.js";\n'
    };
    for (const name of Object.keys(sources)) {
      write(join(modules, name, "package.json"), manifest(name, { depkg: "1.0.0" }));
      write(join(modules, name, "dist/index.js"), `${sources[name]}${LOCAL}`);
      write(join(modules, name, "dist/index.d.ts"), `${declarations[name]}${LOCAL_DECLARATION}`);
      for (const [file, contents] of Object.entries(siblings[name] ?? {})) write(join(modules, name, file), contents);
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
            // The sibling barrel imports the dependency too (ADR 0169).
            reexportImporters: [
              importer,
              ...Object.keys(siblings[name] ?? {})
                .filter(file => file.endsWith("transform.js"))
                .map(file => join(modules, name, file))
            ]
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

  test("a sibling barrel the entry republishes exactly is described as the entry's own re-export is", () => {
    for (const name of ["sibling", "siblingStar"]) {
      expect(closure(documents[name], "local"), name).toEqual(closure(documents.onestep, "local"));
      expect(closure(documents[name], "opaque"), name).toEqual({ closed: [], proposed: [] });
    }
    // A rename on the way is followed to the name the entry publishes.
    expect(closure(documents.siblingRenamed, "local")).toEqual(closure(documents.onestep, "local"));
    expect(closure(documents.siblingRenamed, "renamed")).toEqual({ closed: [], proposed: [] });
  });

  test("a barrel namespace or a cycle between the entry and the barrel still marks every export", () => {
    for (const name of ["siblingNamespace", "siblingCycle"]) {
      expect(closure(documents[name], "local").closed, name).not.toContain("callbacks");
    }
  });

  test("a binding with any other use still marks every export", () => {
    // `held` reads `opaque` at module evaluation; nothing proves which export
    // that reaches, so the ladder keeps its widest answer.
    expect(closure(documents.held, "local").closed).not.toContain("callbacks");
  });
});
