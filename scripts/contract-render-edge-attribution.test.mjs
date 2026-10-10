// Pins ADR 0136: `createComponent(Panel, props)` enters `Panel` exactly as the
// tag `<Panel/>` does, so an obligation inside a private component belongs to
// the exports that render it -- and only an exact renderer and an exact
// component make that edge.
//
// `@tanstack/solid-router@2.0.0-rc.8`'s `Matches.js` is the case:
// `createComponent(Transitioner, {})` is the only entry into `Transitioner`,
// and the reference read as a value escaping into a callee the graph could not
// follow, so every obligation in it marked every export.
//
// The Solid stubs below keep the one declaration the proof depends on
// byte-faithful to `solid-js@2.0.0-rc.9` (`types/client/component.d.ts:73`),
// and `@solidjs/web`'s re-export of it in the shape the published
// `types/client.d.ts` writes (`import { …, createComponent, … } from
// "solid-js"` at `:1`, `export { …, createComponent }` at `:97`). The packages
// under test are untyped JavaScript, so no other Solid signature is read.

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
    "the render edge pin needs fresh native and Type Facts binaries; " +
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
    peerDependencies: { "solid-js": "2.0.0-rc.9", "@solidjs/web": "2.0.0-rc.9" },
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
const SHELL_DECLARATION = "export declare function Shell(props: object): unknown;\n";
const PANEL = 'import { opaque } from "depkg";\nfunction Panel(props) {\n  return opaque(props);\n}\n';
const PANEL_MODULE = `${PANEL}export { Panel };\n`;

function splitPanel() {
  return {
    "dist/index.js":
      'import { createComponent } from "@solidjs/web";\nimport { Panel } from "./panel";\n' +
      `export function Shell(props) {\n  return createComponent(Panel, props);\n}\n${LOCAL}`,
    "dist/index.d.ts": `${SHELL_DECLARATION}${LOCAL_DECLARATION}`,
    "dist/panel.js": PANEL_MODULE,
    "dist/panel.d.ts": "export declare function Panel(props: object): unknown;\n"
  };
}

describe("createComponent renders its component as a tag does", () => {
  let directory;
  const documents = {};

  beforeAll(async () => {
    directory = realpathSync(mkdtempSync(join(tmpdir(), "solid-checker-render-edge-")));
    const modules = join(directory, "node_modules");

    write(
      join(modules, "solid-js/package.json"),
      `${JSON.stringify({
        name: "solid-js",
        version: "2.0.0-rc.9",
        type: "module",
        exports: { ".": { types: "./types/index.d.ts", default: "./dist/solid.js" } }
      })}\n`
    );
    write(join(modules, "solid-js/types/index.d.ts"), 'export * from "./client/component.js";\n');
    write(
      join(modules, "solid-js/types/client/component.d.ts"),
      'import type { Element as SolidElement } from "../types.js";\n' +
        "export type Component<P extends Record<string, any> = {}> = (props: P) => SolidElement;\n" +
        "export declare function createComponent<T extends Record<string, any>>(Comp: Component<T>, props: T, name?: string): SolidElement;\n"
    );
    write(join(modules, "solid-js/types/types.d.ts"), "export type Element = unknown;\n");
    write(
      join(modules, "solid-js/dist/solid.js"),
      "export function createComponent(Comp, props, name) {\n  return Comp(props || {});\n}\n"
    );
    write(
      join(modules, "@solidjs/web/package.json"),
      `${JSON.stringify({
        name: "@solidjs/web",
        version: "2.0.0-rc.9",
        type: "module",
        exports: { ".": { types: "./types/index.d.ts", default: "./dist/web.js" } },
        peerDependencies: { "solid-js": "2.0.0-rc.9" }
      })}\n`
    );
    write(join(modules, "@solidjs/web/types/index.d.ts"), 'export * from "./client.js";\n');
    write(
      join(modules, "@solidjs/web/types/client.d.ts"),
      'import { createComponent } from "solid-js";\nexport { createComponent };\n'
    );
    write(join(modules, "@solidjs/web/dist/web.js"), 'export { createComponent } from "solid-js";\n');

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
      // The router's shape: the renderer imported from `@solidjs/web`.
      rendered: {
        "dist/index.js":
          `import { createComponent } from "@solidjs/web";\n${PANEL}` +
          `export function Shell(props) {\n  return createComponent(Panel, props);\n}\n${LOCAL}`,
        "dist/index.d.ts": `${SHELL_DECLARATION}${LOCAL_DECLARATION}`
      },
      // The same call spelled against `solid-js`, which declares it.
      direct: {
        "dist/index.js":
          `import { createComponent } from "solid-js";\n${PANEL}` +
          `export function Shell(props) {\n  return createComponent(Panel, props);\n}\n${LOCAL}`,
        "dist/index.d.ts": `${SHELL_DECLARATION}${LOCAL_DECLARATION}`
      },
      // A project function that is only *called* createComponent renders
      // nothing the graph may trust: it keeps `Panel` for later.
      impostor: {
        "dist/index.js":
          `${PANEL}const kept = [];\nfunction createComponent(Comp, props) {\n  kept.push(Comp);\n  return props;\n}\n` +
          `export function Shell(props) {\n  return createComponent(Panel, props);\n}\n${LOCAL}`,
        "dist/index.d.ts": `${SHELL_DECLARATION}${LOCAL_DECLARATION}`
      },
      // The renderer is exact but what it renders is a parameter: `Panel`
      // reaches it only as a value handed to `Frame`.
      parameter: {
        "dist/index.js":
          `import { createComponent } from "@solidjs/web";\n${PANEL}` +
          "function Frame(Comp, props) {\n  return createComponent(Comp, props);\n}\n" +
          `export function Shell(props) {\n  return Frame(Panel, props);\n}\n${LOCAL}`,
        "dist/index.d.ts": `${SHELL_DECLARATION}${LOCAL_DECLARATION}`
      },
      // ADR 0158 § 1: the component lives in its own module, named the way a
      // `solid`-condition source build names it (`./panel`), with a sibling
      // `panel.d.ts` TypeScript binds the import to. One file answers the
      // specifier, so the render crosses the split.
      split: splitPanel(),
      // A second candidate for `./panel`: no exact landing, no edge.
      ambiguousSplit: { ...splitPanel(), "dist/panel.jsx": PANEL_MODULE }
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
      // A split package imports the dependency from its component module.
      const importer = join(packageRoot, files["dist/panel.js"] ? "dist/panel.js" : "dist/index.js");
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

  test("a component rendered through createComponent opens the export that renders it", () => {
    expect(closed(documents.control, "local")).toContain("callbacks");
    for (const name of ["rendered", "direct"]) {
      expect(closed(documents[name], "local"), name).toEqual(closed(documents.control, "local"));
      expect(closed(documents[name], "Shell"), name).not.toContain("callbacks");
    }
  });

  test("a component across an extensionless split is rendered when one file answers", () => {
    expect(closed(documents.split, "local")).toEqual(closed(documents.control, "local"));
    expect(closed(documents.split, "Shell")).not.toContain("callbacks");
    expect(closed(documents.ambiguousSplit, "local")).not.toContain("callbacks");
    expect(closed(documents.ambiguousSplit, "Shell")).not.toContain("callbacks");
  });

  test("a renderer by name only, or a rendered parameter, is no edge", () => {
    for (const name of ["impostor", "parameter"]) {
      expect(closed(documents[name], "local"), name).not.toContain("callbacks");
      expect(closed(documents[name], "Shell"), name).not.toContain("callbacks");
    }
  });
});
