// Pins ADR 0139 § 3: an obligation in a function whose only use is an
// argument of a subclass's `super(...)` belongs to that subclass's creators, as
// far as the base's accepted contract says what the base does with the
// argument.
//
// - The base keeps the argument for its members only (a `result-access`
//   item): the obligation opens `returns` of the creators, as ADR 0134's
//   instance-member rule does.
// - The base keeps it and the subclass's constructor names a member of `this`
//   (a construction may reach the member that calls it): the obligation is
//   construction, in every domain of the creators.
// - Anything else -- an open base, an escaping instance, a subclass member that
//   could override what the base's own `this.m(...)` runs, another use of the
//   function -- keeps marking every export.
//
// `@tanstack/solid-router@2.0.0-rc.8`'s `Router` is the measured shape:
// `super(options, getStoreFactory)` on `@tanstack/router-core`'s `RouterCore`.

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
    "the super-argument attribution pin needs fresh native and Type Facts binaries; " +
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

function summary(document, exportName) {
  const artifactCase = document.entrypoints?.["."]?.cases?.[0];
  const found = document.summaries?.[artifactCase?.exports?.[exportName]];
  assert.ok(found, `${exportName} must keep an exact export identity`);
  return found;
}

function closed(document, exportName) {
  return [...(summary(document, exportName).call?.closed ?? [])].sort();
}

const LOCAL = "export function local(handler) {\n  handler();\n}\n";
const LOCAL_DECLARATION = "export declare function local(handler: () => void): void;\n";

// `getStoreFactory`'s shape: a private arrow whose body calls a dependency
// export whose contract is open, handed to the base as `super`'s second
// argument.
function holder(constructor, members = "") {
  return (
    'import { Keeper, Caller, opaque } from "depkg";\n' +
    "var factory = (value) => opaque(value);\n" +
    "function createHolder(options) {\n  return new Holder(options);\n}\n" +
    `var Holder = class extends Keeper {\n  constructor(options) {\n${constructor}  }\n${members}};\n` +
    `export { Holder, createHolder };\n${LOCAL}`
  );
}
const HOLDER_DECLARATION =
  "export declare function createHolder(options: unknown): object;\n" +
  "export declare const Holder: new (options: unknown) => object;\n" +
  LOCAL_DECLARATION;

describe("a function passed to super(...) belongs to the subclass's creators", () => {
  let directory;
  const documents = {};

  beforeAll(async () => {
    directory = realpathSync(mkdtempSync(join(tmpdir(), "solid-checker-super-argument-")));
    const modules = join(directory, "node_modules");

    // `Keeper` keeps its second argument for `run` alone: the generator
    // proposes its `result-access` item and the private graph lane hands the
    // proposal on. `Caller` calls it at construction, which the generator does
    // not describe for a class, so its `callbacks` stays open.
    write(join(modules, "depkg/package.json"), manifest("depkg"));
    write(
      join(modules, "depkg/dist/index.js"),
      "var Keeper = class {\n  constructor(options, callback) {\n    this.callback = callback;\n  }\n" +
        "  run(value) {\n    return this.callback(value);\n  }\n};\n" +
        "var Caller = class {\n  constructor(options, callback) {\n    callback(options);\n  }\n};\n" +
        "const opaque = globalThis.hostFactory();\n" +
        "export { Caller, Keeper, opaque };\n"
    );
    write(
      join(modules, "depkg/dist/index.d.ts"),
      "export declare class Keeper {\n  constructor(options: unknown, callback: (value: unknown) => unknown);\n" +
        "  run(value: unknown): unknown;\n}\n" +
        "export declare class Caller {\n  constructor(options: unknown, callback: (value: unknown) => unknown);\n}\n" +
        "export declare const opaque: (value: unknown) => unknown;\n"
    );

    const sources = {
      control: `${LOCAL}`,
      kept: holder("    super(options, factory);\n"),
      // The subclass constructor names `this`: a construction may reach the
      // base member that calls the kept function.
      touching: holder("    super(options, factory);\n    this.ready = true;\n"),
      escaping: holder("    super(options, factory);\n    globalThis.held = this;\n"),
      overriding: holder("    super(options, factory);\n", "  run(value) {\n    return value;\n  }\n"),
      open: holder("    super(options, factory);\n").replace("extends Keeper", "extends Caller"),
      reused: holder("    super(options, factory);\n").replace(
        `export { Holder, createHolder };`,
        "export const direct = () => factory(1);\nexport { Holder, createHolder };"
      )
    };
    const declarations = {
      control: LOCAL_DECLARATION,
      reused: `${HOLDER_DECLARATION}export declare const direct: () => unknown;\n`
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
    documents.depkg = JSON.parse(readFileSync(dependencyOutput, "utf8"));
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
      const packageRoot = join(modules, name);
      write(join(packageRoot, "package.json"), manifest(name, { depkg: "1.0.0" }));
      write(join(packageRoot, "dist/index.js"), sources[name]);
      write(join(packageRoot, "dist/index.d.ts"), declarations[name] ?? HOLDER_DECLARATION);
      const importer = join(packageRoot, "dist/index.js");
      const options = { quiet: true };
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

  test("the base states what it keeps, and what it does not describe stays open", () => {
    const keeper = summary(documents.depkg, "Keeper").call;
    expect(keeper.closed).toContain("callbacks");
    expect(keeper.operations.map((operation) => operation.at?.event)).toEqual(["result-access"]);
    expect(closed(documents.depkg, "Caller")).not.toContain("callbacks");
  });

  test("kept for members only, the obligation opens only the creators' returns", () => {
    expect(closed(documents.kept, "local")).toEqual(closed(documents.control, "local"));
    expect(closed(documents.kept, "createHolder")).not.toContain("returns");
  });

  test("a constructor that names `this` makes it construction", () => {
    expect(closed(documents.touching, "local")).toEqual(closed(documents.control, "local"));
    expect(closed(documents.touching, "createHolder")).not.toContain("callbacks");
  });

  test("anything the base's contract or the subclass leaves inexact marks every export", () => {
    for (const name of ["escaping", "overriding", "open", "reused"]) {
      expect(closed(documents[name], "local"), name).not.toContain("callbacks");
    }
  });
});
