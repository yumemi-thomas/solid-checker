// Pins ADR 0134: an obligation at, or inside, a module-level class belongs to
// the exports that construct the class or hand its instances out.
//
// - What runs at construction -- a dependency base used only as heritage, the
//   constructor's `super(...)` -- belongs, in every domain, to the class's
//   public names and to the exports that contain its exact `new` sites.
// - What runs when an instance member is invoked -- a closure the constructor
//   installs, a method -- opens `returns` of exactly those exports, and no
//   domain of their own call (owner decision 2026-09-27).
// - Anything the module cannot account for exactly keeps marking every export.
//
// `@tanstack/solid-router@2.0.0-rc.8`'s `route.js` and `router.js` are the
// measured shape: eight construction obligations and one instance-member one,
// each of which marked all 97 root exports.
//
// Amendment of 2026-09-28: a member the constructor can invoke runs at
// construction (`@tanstack/router-core`'s `this.update(...)`), and a member
// whose invokers at construction are not exact -- a base this module cannot
// see, an override a base constructor calls -- keeps marking every export.

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
    "the class attribution pin needs fresh native and Type Facts binaries; " +
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

// The class module, in the shape `route.js` has: a dependency base and a
// `super(...)`. (A member closure beside them is the `inherited` case.)
const THING =
  "function createThing(options) {\n  return new Thing(options);\n}\n" +
  "var Thing = class extends Base {\n  constructor(options) {\n    super(options);\n  }\n};\n";
// A class with no dependency base: only the instance-member obligation.
const PLAIN =
  "function createPlain() {\n  return new Plain();\n}\n" +
  "var Plain = class {\n  constructor() {\n    this.lookup = (id) => opaque(id);\n  }\n};\n";
const DECLARATIONS =
  "export declare function createThing(options: unknown): { lookup(id: unknown): unknown };\n" +
  "export declare const Thing: new (options: unknown) => { lookup(id: unknown): unknown };\n" +
  "export declare function createPlain(): { lookup(id: unknown): unknown };\n";

describe("a class obligation belongs to the exports that construct or hand out the class", () => {
  let directory;
  const documents = {};

  beforeAll(async () => {
    directory = realpathSync(mkdtempSync(join(tmpdir(), "solid-checker-class-attribution-")));
    const modules = join(directory, "node_modules");

    write(join(modules, "depkg/package.json"), manifest("depkg"));
    write(
      join(modules, "depkg/dist/index.js"),
      "export function clean(handler) {\n  handler();\n}\n" +
        "export const Base = globalThis.hostBase();\n" +
        "export const opaque = globalThis.hostFactory();\n"
    );
    write(
      join(modules, "depkg/dist/index.d.ts"),
      "export declare function clean(handler: () => void): void;\n" +
        "export declare const Base: new (options: unknown) => object;\n" +
        "export declare const opaque: (id: unknown) => unknown;\n"
    );

    const imports = 'import { Base, opaque } from "depkg";\n';
    const sources = {
      // No class at all: what `local` is when nothing next to it is open.
      control: `${LOCAL}`,
      classes: `${imports}${THING}${PLAIN}export { Thing, createPlain, createThing };\n${LOCAL}`,
      // The instance escapes its own constructor, so who hands it out is not
      // exact: the member obligation keeps marking every export.
      escaping:
        'import { opaque } from "depkg";\nfunction register(value) {\n  return value;\n}\n' +
        "function createPlain() {\n  return new Plain();\n}\n" +
        "var Plain = class {\n  constructor() {\n    register(this);\n    this.lookup = (id) => opaque(id);\n  }\n};\n" +
        `export { createPlain };\n${LOCAL}`,
      // The constructor installs a member and then calls it, as
      // `RouterCore`'s constructor calls `this.update(...)`.
      invoked:
        'import { opaque } from "depkg";\nfunction createInvoked() {\n  return new Invoked();\n}\n' +
        "var Invoked = class {\n  constructor() {\n    this.lookup = (id) => opaque(id);\n    this.lookup(0);\n  }\n};\n" +
        `export { createInvoked };\n${LOCAL}`,
      // A dependency base's constructor may call the member during
      // `super(...)`, and nothing here says whether it does.
      inherited:
        'import { Base, opaque } from "depkg";\n' +
        "function createThing(options) {\n  return new Thing(options);\n}\n" +
        "var Thing = class extends Base {\n  constructor(options) {\n    super(options);\n" +
        "    this.lookup = (id) => opaque(id);\n  }\n};\n" +
        `export { createThing };\n${LOCAL}`,
      // The base constructor calls a member the subclass overrides, so the
      // override runs while the subclass is constructed.
      overridden:
        'import { opaque } from "depkg";\nfunction createSub() {\n  return new Sub();\n}\n' +
        "var Core = class {\n  constructor() {\n    this.init();\n  }\n  init() {}\n};\n" +
        "var Sub = class extends Core {\n  init() {\n    opaque(1);\n  }\n};\n" +
        `export { createSub };\n${LOCAL}`
    };
    const declarations = {
      control: LOCAL_DECLARATION,
      classes: `${DECLARATIONS}${LOCAL_DECLARATION}`,
      escaping:
        "export declare function createPlain(): { lookup(id: unknown): unknown };\n" +
        LOCAL_DECLARATION,
      invoked:
        "export declare function createInvoked(): { lookup(id: unknown): unknown };\n" +
        LOCAL_DECLARATION,
      inherited:
        "export declare function createThing(options: unknown): { lookup(id: unknown): unknown };\n" +
        LOCAL_DECLARATION,
      overridden: `export declare function createSub(): { init(): void };\n${LOCAL_DECLARATION}`
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

    for (const name of Object.keys(sources)) {
      const packageRoot = join(modules, name);
      write(join(packageRoot, "package.json"), manifest(name, { depkg: "1.0.0" }));
      write(join(packageRoot, "dist/index.js"), sources[name]);
      write(join(packageRoot, "dist/index.d.ts"), declarations[name]);
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

  test("an export beside the classes is described exactly as it is alone", () => {
    expect(closed(documents.control, "local")).toContain("callbacks");
    expect(closed(documents.classes, "local")).toEqual(closed(documents.control, "local"));
  });

  test("construction obligations open the constructing exports", () => {
    // `super(options)` hands the caller's `options` to a base whose contract
    // closes nothing, so `createThing` cannot close `callbacks`.
    expect(closed(documents.classes, "createThing")).not.toContain("callbacks");
  });

  test("an instance-member obligation opens only `returns` of the creators", () => {
    // `createPlain` has no base and no parameter: the member closure is its
    // only open fact, and it must not reach the call's own domains.
    expect(closed(documents.classes, "createPlain")).toContain("callbacks");
    expect(closed(documents.classes, "createPlain")).not.toContain("returns");
  });

  test("an instance that escapes its constructor keeps marking every export", () => {
    expect(closed(documents.escaping, "local")).not.toContain("callbacks");
  });

  test("a member the constructor invokes opens the creators' own call", () => {
    // Attributed exactly: `local` is untouched, and `createInvoked` runs the
    // member's open dependency call inside its own construction.
    expect(closed(documents.invoked, "local")).toEqual(closed(documents.control, "local"));
    expect(closed(documents.invoked, "createInvoked")).not.toContain("callbacks");
  });

  test("a member a base constructor may invoke keeps marking every export", () => {
    expect(closed(documents.inherited, "local")).not.toContain("callbacks");
    expect(closed(documents.overridden, "local")).not.toContain("callbacks");
  });
});
