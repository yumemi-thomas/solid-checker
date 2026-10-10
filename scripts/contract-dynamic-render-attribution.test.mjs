// Pins ADR 0138: a function whose value reaches only `Dynamic`'s `component`
// prop is entered by the renders it reaches, so an obligation inside it
// belongs to the exports containing those renders -- and only an exact value
// flow into an exact `Dynamic` makes that edge.
//
// `@tanstack/solid-router@2.0.0-rc.8`'s `Match.js` is the case:
// `const ResolvedNotFoundBoundary = Solid.createMemo(() => cond() ?
// CatchNotFound : SafeFragment)` is rendered only as `createComponent(Dynamic,
// { get component() { return ResolvedNotFoundBoundary(); } })`, and the
// reference read as a value escaping into a callee the graph could not follow,
// so every obligation of `getNotFound` marked every export.
//
// The Solid stubs below keep the declarations the proof depends on
// byte-faithful to the rc.9 typings: `createComponent`
// (`solid-js` `types/client/component.d.ts:73`), `createMemo` (`solid-js`
// `types/client/hydration.d.ts:114-119`, with the `@solidjs/signals` types it
// names from `dist/types/signals.d.ts:49-50,68,225` and
// `core/constants.d.ts:213-215`), and `Dynamic` with its props
// (`@solidjs/web` `types/index.d.ts:46-54,117`). `MemoOptions` is reduced to
// one field and `JSX` to the two members `Dynamic` names; neither is read by
// the proof. The runtime stubs are not read at all. The packages under test
// are untyped JavaScript, so no other Solid signature is read.

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
    "the Dynamic render pin needs fresh native and Type Facts binaries; " +
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

const IMPORTS = 'import { createComponent, Dynamic } from "@solidjs/web";\nimport { createMemo } from "solid-js";\n';
const LOCAL = "export function local(handler) {\n  handler();\n}\n";
const LOCAL_DECLARATION = "export declare function local(handler: () => void): void;\n";
const SHELL_DECLARATION = "export declare function Shell(props: object): unknown;\n";
const PANEL =
  'import { opaque } from "depkg";\nfunction Panel(props) {\n  return opaque(props);\n}\n' +
  "function Other() {\n  return null;\n}\n";
const MEMO_RENDER =
  "  return createComponent(Dynamic, {\n    get component() {\n      return Selected();\n    }\n  });\n";

describe("Dynamic renders the component its component prop holds", () => {
  let directory;
  const documents = {};

  beforeAll(async () => {
    directory = realpathSync(mkdtempSync(join(tmpdir(), "solid-checker-dynamic-render-")));
    const modules = join(directory, "node_modules");

    write(
      join(modules, "@solidjs/signals/package.json"),
      `${JSON.stringify({
        name: "@solidjs/signals",
        version: "2.0.0-rc.9",
        type: "module",
        exports: { ".": { types: "./dist/types/index.d.ts", default: "./dist/prod/index.js" } }
      })}\n`
    );
    write(
      join(modules, "@solidjs/signals/dist/types/index.d.ts"),
      'export type { Accessor, SourceAccessor, ComputeFunction, MemoOptions, NoInfer } from "./signals.js";\n'
    );
    write(
      join(modules, "@solidjs/signals/dist/types/signals.d.ts"),
      'import type { Refreshable } from "./core/constants.js";\n' +
        "export type Accessor<T> = () => T;\n" +
        "export type SourceAccessor<T> = Refreshable<Accessor<T>>;\n" +
        "export type ComputeFunction<Prev, Next extends Prev = Prev> = (v: Prev) => PromiseLike<Next> | AsyncIterable<Next> | Next;\n" +
        "export interface MemoOptions<T> {\n    name?: string;\n}\n" +
        "export type NoInfer<T extends any> = [T][T extends any ? 0 : never];\n"
    );
    write(
      join(modules, "@solidjs/signals/dist/types/core/constants.d.ts"),
      "export declare const $REFRESH: unique symbol;\n" +
        "export type Refreshable<T> = T & {\n    readonly [$REFRESH]: any;\n};\n"
    );
    write(join(modules, "@solidjs/signals/dist/prod/index.js"), "export {};\n");

    write(
      join(modules, "solid-js/package.json"),
      `${JSON.stringify({
        name: "solid-js",
        version: "2.0.0-rc.9",
        type: "module",
        exports: { ".": { types: "./types/index.d.ts", default: "./dist/solid.js" } },
        dependencies: { "@solidjs/signals": "2.0.0-rc.9" }
      })}\n`
    );
    write(
      join(modules, "solid-js/types/index.d.ts"),
      'export * from "./client/component.js";\nexport { createMemo } from "./client/hydration.js";\n'
    );
    write(
      join(modules, "solid-js/types/client/component.d.ts"),
      'import type { Element as SolidElement } from "../types.js";\n' +
        "export type Component<P extends Record<string, any> = {}> = (props: P) => SolidElement;\n" +
        "export declare function createComponent<T extends Record<string, any>>(Comp: Component<T>, props: T, name?: string): SolidElement;\n"
    );
    write(
      join(modules, "solid-js/types/client/hydration.d.ts"),
      'import { type ComputeFunction, type MemoOptions, type NoInfer, type SourceAccessor } from "@solidjs/signals";\n' +
        "type HydrationMemoOptions<T> = MemoOptions<T>;\n" +
        "export declare const createMemo: {\n" +
        "    <T>(compute: ComputeFunction<NoInfer<T>, T>, options: HydrationMemoOptions<T> & {\n" +
        "        loadingValue: T;\n" +
        "    }): SourceAccessor<T>;\n" +
        "    <T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, options?: HydrationMemoOptions<T>): SourceAccessor<T>;\n" +
        "};\n"
    );
    write(join(modules, "solid-js/types/types.d.ts"), "export type Element = unknown;\n");
    write(
      join(modules, "solid-js/dist/solid.js"),
      "export function createComponent(Comp, props, name) {\n  return Comp(props || {});\n}\n" +
        "export function createMemo(compute) {\n  return compute;\n}\n"
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
    write(
      join(modules, "@solidjs/web/types/index.d.ts"),
      'import { Component } from "solid-js";\nimport type { JSX } from "./jsx.js";\nexport * from "./client.js";\n' +
        "export type IntrinsicElement = Extract<keyof JSX.IntrinsicElements, string>;\n" +
        "export type ValidComponent = IntrinsicElement | Component<any> | (string & {});\n" +
        "export type ComponentProps<T extends ValidComponent> = T extends Component<infer P> ? P : T extends keyof JSX.IntrinsicElements ? JSX.IntrinsicElements[T] : Record<string, unknown>;\n" +
        "export type DynamicProps<T extends ValidComponent, P = ComponentProps<T>> = {\n" +
        "    [K in keyof P]: P[K];\n" +
        "} & {\n" +
        "    component: T | null | undefined | false;\n" +
        "};\n" +
        "export declare function Dynamic<T extends ValidComponent>(props: DynamicProps<T>): JSX.Element;\n"
    );
    write(
      join(modules, "@solidjs/web/types/jsx.d.ts"),
      "export declare namespace JSX {\n  type Element = unknown;\n  interface IntrinsicElements {\n    div: {};\n  }\n}\n"
    );
    write(
      join(modules, "@solidjs/web/types/client.d.ts"),
      'import { createComponent } from "solid-js";\nexport { createComponent };\n'
    );
    write(
      join(modules, "@solidjs/web/dist/web.js"),
      'import { createComponent } from "solid-js";\nexport { createComponent };\n' +
        "export function Dynamic(props) {\n  return createComponent(props.component, props);\n}\n"
    );

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

    const shell = (body) => ({
      "dist/index.js": `${IMPORTS}${PANEL}export function Shell(props) {\n${body}}\n${LOCAL}`,
      "dist/index.d.ts": `${SHELL_DECLARATION}${LOCAL_DECLARATION}`
    });
    const packages = {
      control: { "dist/index.js": LOCAL, "dist/index.d.ts": LOCAL_DECLARATION },
      // The router's shape: a memo choosing between two components, read in
      // the getter the compiler writes for `component={Selected()}`.
      memo: shell(`  const Selected = createMemo(() => props.open ? Panel : Other);\n${MEMO_RENDER}`),
      // `??` as `Match.js:200` writes it, the component the last operand.
      coalesce: shell(`  const Selected = createMemo(() => props.as ?? Panel);\n${MEMO_RENDER}`),
      // A data property, as `<Dynamic component={Panel}/>` may compile.
      direct: shell("  return createComponent(Dynamic, { component: Panel });\n"),
      // The accessor also escapes: whoever holds it can call what it yields.
      escaped: {
        "dist/index.js":
          `${IMPORTS}${PANEL}const kept = [];\nexport function Shell(props) {\n` +
          `  const Selected = createMemo(() => props.open ? Panel : Other);\n  kept.push(Selected);\n${MEMO_RENDER}}\n${LOCAL}`,
        "dist/index.d.ts": `${SHELL_DECLARATION}${LOCAL_DECLARATION}`
      },
      // A holder call that is not a memo: `hold` may call what it is given.
      holder: {
        "dist/index.js":
          `${IMPORTS}${PANEL}function hold(compute) {\n  compute()();\n  return compute;\n}\n` +
          `export function Shell(props) {\n  const Selected = hold(() => Panel);\n${MEMO_RENDER}}\n${LOCAL}`,
        "dist/index.d.ts": `${SHELL_DECLARATION}${LOCAL_DECLARATION}`
      },
      // A project function that is only *named* Dynamic keeps its component.
      impostor: {
        "dist/index.js":
          'import { createComponent } from "@solidjs/web";\n' +
          `${PANEL}const kept = [];\nfunction Dynamic(props) {\n  kept.push(props.component);\n  return null;\n}\n` +
          `export function Shell(props) {\n  return createComponent(Dynamic, { component: Panel });\n}\n${LOCAL}`,
        "dist/index.d.ts": `${SHELL_DECLARATION}${LOCAL_DECLARATION}`
      },
      // The value reaches `Dynamic` only through a parameter.
      parameter: {
        "dist/index.js":
          `${IMPORTS}${PANEL}function Frame(Comp) {\n  return createComponent(Dynamic, { component: Comp });\n}\n` +
          `export function Shell(props) {\n  return Frame(Panel);\n}\n${LOCAL}`,
        "dist/index.d.ts": `${SHELL_DECLARATION}${LOCAL_DECLARATION}`
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
      const importer = join(packageRoot, "dist/index.js");
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

  test("a component Dynamic renders opens only the export that renders it", () => {
    expect(closed(documents.control, "local")).toContain("callbacks");
    for (const name of ["memo", "coalesce", "direct"]) {
      expect(closed(documents[name], "local"), name).toEqual(closed(documents.control, "local"));
      expect(closed(documents[name], "Shell"), name).not.toContain("callbacks");
    }
  });

  test("an escaping accessor, a holder that is no memo, an impostor or a parameter is no edge", () => {
    for (const name of ["escaped", "holder", "impostor", "parameter"]) {
      expect(closed(documents[name], "local"), name).not.toContain("callbacks");
      expect(closed(documents[name], "Shell"), name).not.toContain("callbacks");
    }
  });
});
