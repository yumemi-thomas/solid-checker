import { afterEach, describe, expect, test } from "vitest";
import { Buffer } from "node:buffer";
import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  symlinkSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

import {
  ArtifactResolutionSession,
  ArtifactResolutionError,
  canonicalClosure,
  materializedGeneratedClosureEntry,
  resolvePackageArtifactClosure,
  resolvePackageArtifacts,
  resolvePackageDependencyPlanClosure,
  isCustomCondition,
  isPrivateNamespacedCondition,
  nonModuleTargetExtension,
  resolvePackageExport,
  selectPackageExportTarget,
  selectTypeScriptApi
} from "../scripts/artifact-resolution.mjs";

const roots = [];

afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

function fixture(manifest, files) {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-phase7-"));
  roots.push(root);
  writeFileSync(join(root, "package.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  for (const [path, bytes] of Object.entries(files)) {
    const target = join(root, path);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, bytes);
  }
  return root;
}

function target(root, manifest, conditions, axis = "runtime", resolutionKind = "import") {
  return resolvePackageExport({
    packageRoot: root,
    manifest,
    entrypoint: ".",
    conditions,
    axis,
    resolutionKind
  });
}

describe("standalone package-export resolution", () => {
  test("preserves nested ordered branches for every supported custom condition", () => {
    const manifest = {
      name: "matrix",
      version: "1.0.0",
      exports: {
        ".": {
          types: "./types/index.d.ts",
          browser: {
            development: "./dist/browser-dev.js",
            default: "./dist/browser.js"
          },
          worker: "./dist/worker.js",
          deno: "./dist/deno.js",
          bun: "./dist/bun.js",
          node: {
            require: "./dist/node.cjs",
            import: "./dist/node.js"
          },
          default: "./dist/default.js"
        }
      }
    };
    const files = Object.fromEntries(
      [
        "types/index.d.ts",
        "dist/browser-dev.js",
        "dist/browser.js",
        "dist/worker.js",
        "dist/deno.js",
        "dist/bun.js",
        "dist/node.cjs",
        "dist/node.js",
        "dist/default.js"
      ].map(path => [path, "export {};\n"])
    );
    const root = fixture(manifest, files);

    expect(target(root, manifest, ["browser", "development"]).file.path).toBe(
      join(root, "dist/browser-dev.js")
    );
    expect(target(root, manifest, ["browser"]).file.path).toBe(join(root, "dist/browser.js"));
    for (const condition of ["worker", "deno", "bun"]) {
      expect(target(root, manifest, [condition]).file.path).toBe(join(root, `dist/${condition}.js`));
    }
    expect(target(root, manifest, ["node"], "runtime", "import").file.path).toBe(
      join(root, "dist/node.js")
    );
    expect(target(root, manifest, ["node"], "runtime", "require").file.path).toBe(
      join(root, "dist/node.cjs")
    );
    expect(target(root, manifest, []).file.path).toBe(join(root, "dist/default.js"));

    const trace = target(root, manifest, ["browser", "development"]).trace;
    expect(trace.branch).toBe("/exports/./browser/development");
    expect(trace.steps.map(step => step.condition)).toEqual([
      "subpath",
      "browser",
      "development",
      "target"
    ]);
  });

  test("a nested object that selects nothing backtracks to the next sibling key", () => {
    // Node's PACKAGE_TARGET_RESOLVE continues to the next key when a matched
    // key's own target resolves to nothing, so ["vendor"] here resolves
    // ./index.js. Returning on the first *matching* key and refusing there
    // would report a defect where every real consumer resolves fine -- and
    // disagree with the Rust replay, whose `select_target` backtracks the same
    // way.
    const manifest = {
      name: "nested-backtrack",
      version: "1.0.0",
      exports: {
        ".": {
          vendor: { browser: "./missing.js" },
          default: "./index.js"
        },
        "./blocked": {
          vendor: { browser: null },
          default: "./index.js"
        },
        "./none": { vendor: { browser: "./index.js" } }
      }
    };
    const root = fixture(manifest, {
      "index.js": "export const value = 1;\n",
      "browser.js": "export const value = 2;\n"
    });
    const select = (entrypoint, conditions) =>
      selectPackageExportTarget({ packageRoot: root, manifest, entrypoint, conditions });

    const selected = select(".", ["vendor"]);
    expect(selected.path).toBe(join(root, "index.js"));
    // The abandoned `vendor` branch leaves no trace: neither the hashed steps
    // nor the conditions-taken list may name a condition the surviving
    // resolution never traversed.
    expect(selected.trace.steps.map(step => step.condition)).toEqual([
      "subpath",
      "default",
      "target"
    ]);
    expect(selected.conditions).toEqual(["default"]);

    // With the nested condition active the inner branch wins, and a target the
    // package does not ship is reported as absent rather than falling through
    // to `default`: a missing file is a property of the package, not an
    // unmatched condition.
    const inner = select(".", ["vendor", "browser"]);
    expect(inner.path).toBe(join(root, "missing.js"));
    expect(inner.exists).toBe(false);
    expect(inner.conditions).toEqual(["vendor", "browser"]);

    // A `null` target blocks; it never backtracks.
    expect(() => select("./blocked", ["vendor", "browser"])).toThrow(/blocked by a null/);

    // And an object that yields nothing at all still refuses.
    expect(() => select("./none", ["vendor"])).toThrow(
      /selects no active package-export condition/
    );
  });

  test("uses manifest key order rather than caller condition order for default and custom branches", () => {
    const defaultFirst = {
      name: "ordered-default",
      version: "1.0.0",
      exports: {
        ".": {
          default: "./dist/default.js",
          browser: "./dist/browser.js"
        }
      }
    };
    const defaultRoot = fixture(defaultFirst, {
      "dist/default.js": "export const selected = 'default';\n",
      "dist/browser.js": "export const selected = 'browser';\n"
    });
    expect(target(defaultRoot, defaultFirst, ["browser"]).file.path).toBe(
      join(defaultRoot, "dist/default.js")
    );

    const customFirst = {
      name: "ordered-custom",
      version: "1.0.0",
      exports: {
        ".": {
          edge: "./dist/edge.js",
          import: "./dist/import.js",
          default: "./dist/default.js"
        }
      }
    };
    const customRoot = fixture(customFirst, {
      "dist/edge.js": "export const selected = 'edge';\n",
      "dist/import.js": "export const selected = 'import';\n",
      "dist/default.js": "export const selected = 'default';\n"
    });
    for (const conditions of [["import", "edge"], ["edge", "import"]]) {
      expect(target(customRoot, customFirst, conditions).file.path).toBe(
        join(customRoot, "dist/edge.js")
      );
    }
  });

  test("binds runtime and declarations independently", () => {
    const manifest = {
      name: "independent",
      version: "1.0.0",
      exports: {
        ".": {
          types: "./types/public.d.ts",
          import: "./dist/public.js",
          default: "./dist/public.js"
        }
      }
    };
    const root = fixture(manifest, {
      "types/public.d.ts": "export declare const value: number;\n",
      "dist/public.js": "export const value = 1;\n"
    });
    const runtime = target(root, manifest, []);
    const declarations = target(root, manifest, [], "declarations");
    expect(runtime.file.path).toBe(join(root, "dist/public.js"));
    expect(declarations.file.path).toBe(join(root, "types/public.d.ts"));
    expect(runtime.trace.branch).toBe("/exports/./import");
    expect(declarations.trace.branch).toBe("/exports/./types");
  });

  test("uses node pattern precedence and refuses zero matches and invalid targets", () => {
    const manifest = {
      name: "patterns",
      version: "1.0.0",
      exports: {
        "./features/*": "./dist/features/*.js",
        "./features/private/*": "./dist/private/*.js"
      }
    };
    const root = fixture(manifest, {
      "dist/features/item.js": "export {};\n",
      "dist/private/item.js": "export {};\n"
    });
    const selected = resolvePackageExport({
      packageRoot: root,
      manifest,
      entrypoint: "./features/private/item",
      conditions: [],
      axis: "runtime"
    });
    expect(selected.file.path).toBe(join(root, "dist/private/item.js"));
    expect(() =>
      resolvePackageExport({
        packageRoot: root,
        manifest,
        entrypoint: "./missing",
        conditions: []
      })
    ).toThrowError(ArtifactResolutionError);

    expect(() =>
      resolvePackageExport({
        packageRoot: root,
        manifest: { exports: { ".": "../outside.js" } },
        entrypoint: "."
      })
    ).toThrow(/must start with/);
    expect(() =>
      resolvePackageExport({
        packageRoot: root,
        manifest: { exports: { ".": "./dist/item.js", browser: "./dist/browser.js" } },
        entrypoint: "."
      })
    ).toThrow(/cannot mix/);
    expect(() =>
      resolvePackageExport({
        packageRoot: root,
        manifest: { exports: { ".": "./dist/%2e%2e/outside.js" } },
        entrypoint: "."
      })
    ).toThrow(/invalid segment/);
  });

  test("legacy runtime resolution prefers a present module target over main", () => {
    const manifest = {
      name: "legacy-dual",
      version: "1.0.0",
      type: "module",
      main: "dist/index.cjs",
      module: "dist/index.js",
      types: "dist/index.d.ts"
    };
    const root = fixture(manifest, {
      "dist/index.cjs": "exports.observe = () => {};\n",
      "dist/index.js": "export const observe = () => {};\n",
      "dist/index.d.ts": "export declare const observe: () => void;\n"
    });

    const runtime = target(root, manifest, []);
    expect(runtime.file.path).toBe(join(root, "dist/index.js"));
    expect(runtime.trace).toEqual({
      branch: "legacy:module",
      steps: [{ condition: "module", target: "dist/index.js" }]
    });

    // The declarations axis is untouched: `module` never names a typing.
    const declarations = target(root, manifest, [], "declarations");
    expect(declarations.file.path).toBe(join(root, "dist/index.d.ts"));
    expect(declarations.trace.branch).toBe("legacy:types");
  });

  test("legacy runtime resolution falls back to main when module is unusable", () => {
    const files = { "dist/index.js": "export const observe = () => {};\n" };
    const root = fixture({ name: "legacy-fallback", version: "1.0.0" }, files);
    for (const declared of [
      undefined,
      "dist/absent.js",
      "../outside/index.js",
      true,
      null
    ]) {
      const manifest = {
        name: "legacy-fallback",
        version: "1.0.0",
        type: "module",
        main: "dist/index.js",
        ...(declared === undefined ? {} : { module: declared })
      };
      const runtime = target(root, manifest, []);
      expect(runtime.file.path).toBe(join(root, "dist/index.js"));
      expect(runtime.trace).toEqual({
        branch: "legacy:main",
        steps: [{ condition: "main", target: "dist/index.js" }]
      });
    }

    const indexed = fixture({ name: "legacy-index", version: "1.0.0" }, {
      "index.js": "export const observe = () => {};\n"
    });
    const runtime = target(
      indexed,
      { name: "legacy-index", version: "1.0.0", module: "dist/absent.js" },
      []
    );
    expect(runtime.file.path).toBe(join(indexed, "index.js"));
    expect(runtime.trace.branch).toBe("legacy:index");
  });
});

describe("exact artifact records and closure", () => {
  test("one transaction acquires identical semantic roots once while retaining condition traces", () => {
    const manifest = {
      name: "condition-alias",
      version: "1.0.0",
      exports: {
        ".": {
          types: "./index.d.ts",
          browser: "./index.js",
          import: "./index.js"
        }
      }
    };
    const root = fixture(manifest, {
      "index.js": "export const value = 1;\n",
      "index.d.ts": "export declare const value: 1;\n"
    });
    const session = new ArtifactResolutionSession();
    const resolveCase = conditions =>
      session.resolve({
        importer: join(root, "consumer.mjs"),
        specifier: "condition-alias",
        packageRoot: root,
        conditions,
        integrity: "sha512:test"
      });

    const unconditioned = resolveCase([]);
    const browser = resolveCase(["browser"]);

    expect(unconditioned.runtimeTrace.branch).toBe("/exports/./import");
    expect(browser.runtimeTrace.branch).toBe("/exports/./browser");
    expect(browser.exports).toEqual(unconditioned.exports);
    expect(browser.closure).toEqual(unconditioned.closure);
    expect(session.statistics()).toEqual({
      requests: 2,
      semanticAcquisitions: 1,
      semanticCacheHits: 1,
      semanticCacheInvalidations: 0,
      moduleDescriptionsParsed: 2,
      typeScriptProgramsCreated: 0
    });
  });

  test("transaction reuse invalidates when a transitive closure member changes", () => {
    const manifest = {
      name: "mutable-closure",
      version: "1.0.0",
      exports: { ".": { types: "./index.d.ts", import: "./index.js" } }
    };
    const root = fixture(manifest, {
      "index.js": 'export { value } from "./leaf.js";\n',
      "leaf.js": "export const value = 1;\n",
      "index.d.ts": 'export { value } from "./leaf.d.ts";\n',
      "leaf.d.ts": "export declare const value: 1;\n"
    });
    const session = new ArtifactResolutionSession();
    const resolveCase = () =>
      session.resolve({
        importer: join(root, "consumer.mjs"),
        specifier: "mutable-closure",
        packageRoot: root,
        integrity: "sha512:test"
      });

    const first = resolveCase();
    writeFileSync(join(root, "leaf.js"), "export const value = 2;\n");
    const second = resolveCase();

    expect(second.closure.digest).not.toBe(first.closure.digest);
    expect(session.statistics()).toEqual({
      requests: 2,
      semanticAcquisitions: 2,
      semanticCacheHits: 0,
      semanticCacheInvalidations: 1,
      moduleDescriptionsParsed: 5,
      typeScriptProgramsCreated: 0
    });
  });

  test("distinct semantic roots parse shared transitive modules once per transaction", () => {
    const manifest = {
      name: "shared-closure",
      version: "1.0.0",
      exports: {
        ".": { types: "./index.d.ts", import: "./index.js" },
        "./other": { types: "./other.d.ts", import: "./other.js" }
      }
    };
    const root = fixture(manifest, {
      "index.js": 'let local = 0; local = 1; export { value } from "./shared.js";\n',
      "index.d.ts": 'export { value } from "./shared.d.ts";\n',
      "other.js": 'let local = 0; local = 1; export { value } from "./shared.js";\n',
      "other.d.ts": 'export { value } from "./shared.d.ts";\n',
      "shared.js": "let local = 0; local = 1; export const value = 1;\n",
      "shared.d.ts": "export declare const value: 1;\n"
    });
    const session = new ArtifactResolutionSession();
    const resolveCase = specifier =>
      session.resolve({
        importer: join(root, "consumer.mjs"),
        specifier,
        packageRoot: root,
        integrity: "sha512:test"
      });

    resolveCase("shared-closure");
    resolveCase("shared-closure/other");

    expect(session.statistics().moduleDescriptionsParsed).toBe(6);
    expect(session.statistics().typeScriptProgramsCreated).toBe(1);
  });

  test("uses the shared typed closure digest framing", () => {
    expect(canonicalClosure().digest).toBe(
      "sha256:19575d19c2fadca45b8704b31f09949362bfb667a45fe12b9708825bb4aad020"
    );
  });

  test("binds local runtime and declaration re-exports to different targets", () => {
    const manifest = {
      name: "bindings",
      version: "1.0.0",
      exports: {
        ".": {
          types: "./types/index.d.ts",
          import: "./dist/index.js"
        }
      }
    };
    const root = fixture(manifest, {
      "dist/index.js": 'export { internal as publicName } from "./impl.js";\n',
      "dist/impl.js": "export const internal = 1;\n",
      "types/index.d.ts": 'export { Declared as publicName } from "./impl.d.ts";\n',
      "types/impl.d.ts": "export declare const Declared: number;\n"
    });
    const record = resolvePackageArtifacts({
      importer: join(root, "consumer.mjs"),
      specifier: "bindings",
      packageRoot: root,
      integrity: "sha512:test"
    });
    expect(record.exports.publicName.runtime.module.path).toBe(join(root, "dist/impl.js"));
    expect(record.exports.publicName.runtime.exportName).toBe("internal");
    expect(record.exports.publicName.declarations.module.path).toBe(join(root, "types/impl.d.ts"));
    expect(record.exports.publicName.declarations.exportName).toBe("Declared");
    expect(record.closure.entries.map(entry => [entry.role, entry.path])).toContainEqual([
      "manifest",
      "./package.json"
    ]);
  });

  test("dependency planning retains closure independently of unresolved external exports", () => {
    const manifest = {
      name: "external-frontier",
      version: "1.0.0",
      exports: { ".": { types: "./index.d.ts", import: "./index.js" } }
    };
    const root = fixture(manifest, {
      "index.js": 'export * from "external-runtime";\n',
      "index.d.ts": 'export * from "external-types/subpath";\n'
    });

    const artifact = resolvePackageArtifacts({
      importer: join(root, "consumer.mjs"),
      specifier: "external-frontier",
      packageRoot: root,
      integrity: "sha512:test"
    });
    expect(artifact.exports).toEqual({});

    const planned = resolvePackageArtifactClosure({
      importer: join(root, "consumer.mjs"),
      specifier: "external-frontier",
      packageRoot: root,
      integrity: "sha512:test"
    });
    expect(planned.closure.hazards.map(hazard => hazard.source)).toEqual([
      "./index.d.ts:external-types/subpath",
      "./index.js:external-runtime"
    ]);
    expect(planned.externalDependencies).toEqual([
      {
        axis: "declarations",
        importerPath: "./index.d.ts",
        kind: "reexport",
        specifier: "external-types/subpath"
      },
      {
        axis: "runtime",
        importerPath: "./index.js",
        kind: "reexport",
        specifier: "external-runtime"
      }
    ]);
    const graph = resolvePackageDependencyPlanClosure({
      importer: join(root, "consumer.mjs"),
      specifier: "external-frontier",
      packageRoot: root,
      integrity: "sha512:test"
    });
    expect(graph.closure.hazards.map(hazard => hazard.source)).toEqual([
      "./index.d.ts:external-types/subpath",
      "./index.js:external-runtime"
    ]);
    expect(graph.closure.frontiers).toEqual([]);
  });

  test("dependency graph planning follows exact package imports aliases inside the authenticated root", () => {
    const manifest = {
      name: "package-imports",
      version: "1.0.0",
      exports: { ".": { types: "./index.d.ts", import: "./index.js" } },
      imports: {
        "#server-fn-resolver": {
          browser: "./dist/browser-resolver.js",
          default: "./dist/default-resolver.js"
        }
      }
    };
    const root = fixture(manifest, {
      "index.js": 'export { resolve } from "#server-fn-resolver";\n',
      "index.d.ts": 'export { resolve } from "#server-fn-resolver";\n',
      "dist/browser-resolver.js": "export const resolve = () => 'browser';\n",
      "dist/default-resolver.js": "export const resolve = () => 'default';\n"
    });

    // The proposal closure deliberately refuses instead: the Rust certifier has
    // no imports-map support, so a proposal whose closure walked into
    // `./dist/browser-resolver.js` could only be rejected on replay. See the
    // parity test below. Graph PLANNING is a different operation with no
    // replayed closure, so it still follows the alias exactly.
    expect(() =>
      resolvePackageArtifacts({
        importer: join(root, "consumer.mjs"),
        specifier: "package-imports",
        packageRoot: root,
        conditions: ["browser"],
        integrity: "sha512:test"
      })
    ).toThrow(/package imports-map target .* certifier replay does not support imports maps yet/);

    const graph = resolvePackageDependencyPlanClosure({
      importer: join(root, "consumer.mjs"),
      specifier: "package-imports",
      packageRoot: root,
      conditions: ["browser"],
      integrity: "sha512:test"
    });

    expect(graph.closure.hazards).toEqual([]);
    expect(graph.closure.entries).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ role: "runtime", path: "./dist/browser-resolver.js" }),
        expect.objectContaining({ role: "declaration", path: "./dist/browser-resolver.js" })
      ])
    );
    expect(graph.closure.entries).not.toEqual(
      expect.arrayContaining([
        expect.objectContaining({ path: "./dist/default-resolver.js" })
      ])
    );
  });

  test("a matched package imports target refuses the proposal closure until Rust replays imports maps", () => {
    const manifest = {
      name: "package-imports-parity",
      version: "1.0.0",
      exports: { ".": { types: "./index.d.ts", import: "./index.js" } },
      imports: { "#dep": { default: "./dist/dep.js" } }
    };
    const root = fixture(manifest, {
      "index.js": 'export { helper } from "#dep";\n',
      "index.d.ts": 'export { helper } from "#dep";\n',
      "dist/dep.js": "export const helper = () => 1;\n"
    });
    const request = {
      importer: join(root, "consumer.mjs"),
      specifier: "package-imports-parity",
      packageRoot: root,
      integrity: "sha512:test"
    };

    // `#dep` is matched by `default` under every partition, so this closure
    // would contain ./dist/dep.js while the certifier's replay -- which has no
    // `imports` field on its snapshot manifest and treats every `#specifier` as
    // External -- would not. That is a closure mismatch on replay, so the
    // generator must refuse the artifact case rather than emit a proposal that
    // cannot certify. Removing this refusal requires imports-map replay on the
    // Rust side; see docs/precision-backlog.md.
    let refusal;
    try {
      resolvePackageArtifacts(request);
    } catch (error) {
      refusal = error;
    }
    expect(refusal).toBeInstanceOf(ArtifactResolutionError);
    expect(refusal.code).toBe("package-imports-unsupported");
    expect(refusal.message).toBe(
      "package imports-map target ./dist/dep.js resolves into the closure; " +
        "certifier replay does not support imports maps yet"
    );
    expect(() => resolvePackageArtifactClosure(request)).toThrow(
      /certifier replay does not support imports maps yet/
    );
  });

  test("a nested dependency resolves package imports against its own manifest", () => {
    const manifest = {
      name: "package-imports-owner",
      version: "1.0.0",
      exports: { ".": { types: "./index.d.ts", import: "./index.js" } }
    };
    const dependencyManifest = {
      name: "nested-dependency",
      version: "1.0.0",
      imports: { "#dependency-entry": "./entry.js" }
    };
    const root = fixture(manifest, {
      "index.js": 'export * from "./node_modules/nested-dependency/index.js";\n',
      "index.d.ts": 'export * from "./node_modules/nested-dependency/index.d.ts";\n',
      "node_modules/nested-dependency/package.json":
        `${JSON.stringify(dependencyManifest, null, 2)}\n`,
      "node_modules/nested-dependency/index.js":
        'export { dependencyValue } from "#dependency-entry";\n',
      "node_modules/nested-dependency/index.d.ts":
        'export { dependencyValue } from "#dependency-entry";\n',
      "node_modules/nested-dependency/entry.js":
        "export const dependencyValue = 1;\n"
    });
    const request = {
      importer: join(root, "consumer.mjs"),
      specifier: "package-imports-owner",
      packageRoot: root,
      integrity: "sha512:test"
    };

    let refusal;
    try {
      resolvePackageArtifacts(request);
    } catch (error) {
      refusal = error;
    }
    expect(refusal).toBeInstanceOf(ArtifactResolutionError);
    expect(refusal.code).toBe("package-imports-unsupported");
    expect(refusal.message).toContain(
      "node_modules/nested-dependency/entry.js resolves into the closure"
    );
  });

  test("a nested dependency cannot inherit package imports from its parent package", () => {
    const manifest = {
      name: "package-imports-parent",
      version: "1.0.0",
      exports: { ".": { types: "./index.d.ts", import: "./index.js" } },
      imports: { "#parent-only": "./parent-entry.js" }
    };
    const root = fixture(manifest, {
      "index.js": 'export * from "./node_modules/nested-dependency/index.js";\n',
      "index.d.ts": 'export * from "./node_modules/nested-dependency/index.d.ts";\n',
      "parent-entry.js": "export const dependencyValue = 1;\n",
      "node_modules/nested-dependency/package.json": `${JSON.stringify({
        name: "nested-dependency",
        version: "1.0.0"
      }, null, 2)}\n`,
      "node_modules/nested-dependency/index.js":
        'export { dependencyValue } from "#parent-only";\n',
      "node_modules/nested-dependency/index.d.ts":
        'export { dependencyValue } from "#parent-only";\n'
    });

    expect(() =>
      resolvePackageArtifacts({
        importer: join(root, "consumer.mjs"),
        specifier: "package-imports-parent",
        packageRoot: root,
        integrity: "sha512:test"
      })
    ).toThrow(
      expect.objectContaining({
        code: "package-import-not-defined",
        message: "#parent-only is not defined by the package imports map"
      })
    );
  });

  test("an unmatched package imports specifier stays an open frontier, never an external dependency", () => {
    const manifest = {
      name: "package-imports-open",
      version: "1.0.0",
      exports: { ".": { types: "./index.d.ts", import: "./index.js" } },
      imports: { "#platform": { browser: "./dist/browser.js", node: "./dist/node.js" } }
    };
    const root = fixture(manifest, {
      "index.js": 'import "#platform";\nexport const thing = 1;\n',
      "index.d.ts": "export declare const thing: number;\n",
      "dist/browser.js": "globalThis.platform = 'browser';\n",
      "dist/node.js": "globalThis.platform = 'node';\n"
    });
    const request = {
      importer: join(root, "consumer.mjs"),
      specifier: "package-imports-open",
      packageRoot: root,
      integrity: "sha512:test"
    };

    // No environment condition is active, so no arm is selected. That is
    // unknown, not absent: the artifact case still certifies, with the same
    // opaque frontier an unaccepted bare dependency records.
    const artifact = resolvePackageArtifacts(request);
    expect(artifact.closure.hazards).toEqual([
      expect.objectContaining({
        kind: "unaccepted-external-dependency",
        source: "./index.js:#platform"
      })
    ]);
    expect(artifact.closure.entries).not.toEqual(
      expect.arrayContaining([expect.objectContaining({ path: "./dist/browser.js" })])
    );

    // And it is emphatically NOT an external dependency: `#platform` names no
    // package, so a census row for it would make the external locator derive a
    // nonsense package name from the specifier.
    const planned = resolvePackageArtifactClosure(request);
    expect(planned.externalDependencies).toEqual([]);
    expect(planned.closure.hazards).toEqual([
      expect.objectContaining({
        kind: "unaccepted-external-dependency",
        source: "./index.js:#platform"
      })
    ]);
  });

  test("dependency graph planning leaves an unmatched package imports condition open", () => {
    const manifest = {
      name: "package-imports-unmatched",
      version: "1.0.0",
      exports: { ".": { types: "./index.d.ts", import: "./index.js" } },
      imports: {
        "#platform": {
          browser: "./dist/browser.js",
          node: "./dist/node.js"
        }
      }
    };
    const root = fixture(manifest, {
      "index.js": 'import "#platform";\nexport const thing = 1;\n',
      "index.d.ts": "export declare const thing: number;\n",
      "dist/browser.js": "globalThis.platform = 'browser';\n",
      "dist/node.js": "globalThis.platform = 'node';\n"
    });

    // No environment condition is active, so this census row cannot say which
    // arm a consumer selects. That is an open frontier, not proof that no
    // runtime module executes, and it must not refuse the artifact case.
    const graph = resolvePackageDependencyPlanClosure({
      importer: join(root, "consumer.mjs"),
      specifier: "package-imports-unmatched",
      packageRoot: root,
      integrity: "sha512:test"
    });
    expect(graph.closure.hazards).toEqual([
      expect.objectContaining({
        kind: "unaccepted-external-dependency",
        source: "./index.js:#platform"
      })
    ]);
    expect(graph.closure.entries).not.toEqual(
      expect.arrayContaining([expect.objectContaining({ path: "./dist/browser.js" })])
    );
    expect(graph.closure.entries).not.toEqual(
      expect.arrayContaining([expect.objectContaining({ path: "./dist/node.js" })])
    );

    // A condition that does select an arm still pulls it into the closure.
    const selected = resolvePackageDependencyPlanClosure({
      importer: join(root, "consumer.mjs"),
      specifier: "package-imports-unmatched",
      packageRoot: root,
      conditions: ["node"],
      integrity: "sha512:test"
    });
    expect(selected.closure.hazards).toEqual([]);
    expect(selected.closure.entries).toEqual(
      expect.arrayContaining([expect.objectContaining({ path: "./dist/node.js" })])
    );
  });

  test("a query-suffixed relative specifier is an opaque asset import, not a missing module", () => {
    const manifest = {
      name: "resource-query",
      version: "1.0.0",
      exports: { ".": { types: "./index.d.ts", import: "./index.js" } }
    };
    const root = fixture(manifest, {
      // The same shipped file, imported both ways. `?raw` binds the loader's
      // product -- the file's source text -- so it is not this module's exports
      // and must not be walked into; the plain import still is.
      "index.js":
        'import source from "./shipped.js?raw";\nimport { run } from "./shipped.js";\nexport const thing = [source, run];\n',
      "index.d.ts": "export declare const thing: unknown[];\n",
      "shipped.js": "export const run = callback => callback();\n"
    });

    const artifact = resolvePackageArtifacts({
      importer: join(root, "consumer.mjs"),
      specifier: "resource-query",
      packageRoot: root,
      integrity: "sha512:test"
    });
    expect(artifact.closure.hazards).toEqual([
      expect.objectContaining({
        kind: "unaccepted-external-dependency",
        source: "./index.js:./shipped.js?raw",
        affectedDomains: expect.arrayContaining(["callbacks", "returns"])
      })
    ]);
    // The module import still reaches the file; the query-suffixed one adds no
    // second edge and no resolution input for a literal `shipped.js?raw`.
    expect(
      artifact.closure.entries.filter(entry => entry.path.startsWith("./shipped.js"))
    ).toEqual([expect.objectContaining({ role: "runtime", path: "./shipped.js" })]);

    // A relative specifier never names a package, so it stays out of the
    // external dependency census that acquires accepted dependencies.
    const planned = resolvePackageArtifactClosure({
      importer: join(root, "consumer.mjs"),
      specifier: "resource-query",
      packageRoot: root,
      integrity: "sha512:test"
    });
    expect(planned.externalDependencies).toEqual([]);
    expect(planned.closure.hazards.map(hazard => hazard.source)).toEqual([
      "./index.js:./shipped.js?raw"
    ]);

    const graph = resolvePackageDependencyPlanClosure({
      importer: join(root, "consumer.mjs"),
      specifier: "resource-query",
      packageRoot: root,
      integrity: "sha512:test"
    });
    expect(graph.closure.hazards).toEqual([
      expect.objectContaining({
        kind: "unaccepted-external-dependency",
        source: "./index.js:./shipped.js?raw"
      })
    ]);
  });

  test("query and fragment suffixes are opaque wherever they appear, and only when non-empty", () => {
    const manifest = {
      name: "resource-suffix-shapes",
      version: "1.0.0",
      exports: { ".": { types: "./index.d.ts", import: "./index.js" } },
      imports: { "#platform": "./dist/platform.js" }
    };
    const files = {
      "index.d.ts": "export declare const thing: number;\n",
      "dist/platform.js": "export const platform = 1;\n"
    };

    // Every suffixed specifier is opaque: an absent query target no more
    // resolves than a present one, a fragment behaves like a query, a `#`
    // imports specifier carrying a query is not looked up in the imports map,
    // and a bare specifier with a suffix names no entrypoint to acquire.
    for (const specifier of [
      "./absent.js?raw",
      "./dist/platform.js?url",
      "./dist/platform.js#fragment",
      "#platform?raw",
      "external-pkg/theme.css?inline"
    ]) {
      const root = fixture(manifest, {
        ...files,
        "index.js": `import "${specifier}";\nexport const thing = 1;\n`
      });
      const planned = resolvePackageArtifactClosure({
        importer: join(root, "consumer.mjs"),
        specifier: "resource-suffix-shapes",
        packageRoot: root,
        integrity: "sha512:test"
      });
      expect(planned.closure.hazards).toEqual([
        expect.objectContaining({
          kind: "unaccepted-external-dependency",
          source: `./index.js:${specifier}`
        })
      ]);
      expect(planned.externalDependencies).toEqual([]);
      expect(planned.closure.entries.map(entry => entry.path)).not.toContain("./dist/platform.js");
    }

    // An introducer with nothing after it is not a suffix: `./absent.js?` stays
    // on the ordinary path, where a specifier with no file still refuses.
    for (const specifier of ["./absent.js", "./absent.js?", "./absent.js#"]) {
      const root = fixture(manifest, {
        ...files,
        "index.js": `import "${specifier}";\nexport const thing = 1;\n`
      });
      expect(() =>
        resolvePackageArtifactClosure({
          importer: join(root, "consumer.mjs"),
          specifier: "resource-suffix-shapes",
          packageRoot: root,
          integrity: "sha512:test"
        })
      ).toThrow(/local closure module .* was not found/);
    }
  });

  test("an unsuffixed relative specifier with no file still refuses the artifact case", () => {
    const manifest = {
      name: "missing-local-module",
      version: "1.0.0",
      exports: { ".": { types: "./index.d.ts", import: "./index.js" } }
    };
    const root = fixture(manifest, {
      "index.js": 'import { run } from "./absent.js";\nexport const thing = run;\n',
      "index.d.ts": "export declare const thing: unknown;\n"
    });
    const request = {
      importer: join(root, "consumer.mjs"),
      specifier: "missing-local-module",
      packageRoot: root,
      integrity: "sha512:test"
    };
    expect(() => resolvePackageArtifacts(request)).toThrow(
      /local closure module \.\/absent\.js from .* was not found/
    );
    expect(() => resolvePackageDependencyPlanClosure(request)).toThrow(
      /local closure module \.\/absent\.js from .* was not found/
    );
  });

  test("dependency graph planning refuses a package imports specifier the map never defines", () => {
    const manifest = {
      name: "package-imports-undefined",
      version: "1.0.0",
      exports: { ".": { types: "./index.d.ts", import: "./index.js" } },
      imports: { "#other": "./dist/other.js" }
    };
    const root = fixture(manifest, {
      "index.js": 'import "#platform";\nexport const thing = 1;\n',
      "index.d.ts": "export declare const thing: number;\n",
      "dist/other.js": "export const other = 1;\n"
    });

    expect(() =>
      resolvePackageDependencyPlanClosure({
        importer: join(root, "consumer.mjs"),
        specifier: "package-imports-undefined",
        packageRoot: root,
        integrity: "sha512:test"
      })
    ).toThrow(/#platform is not defined by the package imports map/);
  });

  test("dependency graph planning stops at an exact require binding frontier", () => {
    const manifest = {
      name: "require-frontier",
      version: "1.0.0",
      exports: { ".": { types: "./index.d.ts", import: "./index.js" } }
    };
    const root = fixture(manifest, {
      "index.js": 'const value = require("external-cjs"); export { value };\n',
      "index.d.ts": "export declare const value: unknown;\n"
    });
    const graph = resolvePackageDependencyPlanClosure({
      importer: join(root, "consumer.mjs"),
      specifier: "require-frontier",
      packageRoot: root,
      integrity: "sha512:test"
    });
    expect(graph.closure.hazards).toEqual([]);
    expect(graph.closure.frontiers).toEqual([
      expect.objectContaining({
        kind: "semantic-require-binding",
        source: "./index.js:14-37",
        specifier: "external-cjs"
      })
    ]);
  });

  test("binds a lowered runtime variable to its declaration enum", () => {
    const manifest = {
      name: "lowered-enum",
      version: "1.0.0",
      exports: { ".": { types: "./index.d.ts", import: "./index.js" } }
    };
    const root = fixture(manifest, {
      "index.js":
        "export var EventType; (function (EventType) { EventType[EventType['Click'] = 0] = 'Click'; })(EventType || (EventType = {}));\n",
      "index.d.ts": "export declare enum EventType { Click = 0 }\n"
    });

    const record = resolvePackageArtifacts({
      importer: join(root, "consumer.mjs"),
      specifier: "lowered-enum",
      packageRoot: root,
      integrity: "sha512:test"
    });

    expect(record.exports.EventType.runtime.module.path).toBe(join(root, "index.js"));
    expect(record.exports.EventType.runtime.exportName).toBe("EventType");
    expect(record.exports.EventType.declarations.module.path).toBe(join(root, "index.d.ts"));
    expect(record.exports.EventType.declarations.exportName).toBe("EventType");
  });

  test("excludes explicit type-only re-exports from a source artifact's runtime surface", () => {
    const manifest = {
      name: "source-types",
      version: "1.0.0",
      exports: { ".": { types: "./index.ts", import: "./index.ts" } }
    };
    const root = fixture(manifest, {
      "index.ts": 'export { value, type Props } from "./impl";\n',
      "impl.ts": "export const value = 1; export interface Props { label: string }\n"
    });

    const record = resolvePackageArtifacts({
      importer: join(root, "consumer.mjs"),
      specifier: "source-types",
      packageRoot: root,
      integrity: "sha512:test"
    });

    expect(Object.keys(record.exports)).toEqual(["value"]);
  });

  test("records accepted external contracts and opens opaque frontiers without discarding local files", () => {
    const manifest = {
      name: "frontier",
      version: "1.0.0",
      exports: { ".": { types: "./index.d.ts", import: "./index.js" } }
    };
    const root = fixture(manifest, {
      "index.js":
        'import { helper } from "accepted"; import("./chunk.js"); import(name); eval(code); helper(); export const value = 1;\n',
      "chunk.js": "export const chunk = 1;\n",
      "index.d.ts": 'import type { Helper } from "accepted"; export declare const value: Helper;\n'
    });
    const record = resolvePackageArtifacts({
      importer: join(root, "consumer.mjs"),
      specifier: "frontier",
      packageRoot: root,
      integrity: "sha512:test",
      acceptedDependencies: {
        accepted: {
          packageName: "accepted",
          artifactCase: "artifact-case:accepted",
          acceptedContractDigest: `sha256:${"1".repeat(64)}`
        }
      }
    });
    expect(record.closure.dependencies).toHaveLength(1);
    expect(record.closure.entries.some(entry => entry.role === "literal-dynamic-chunk")).toBe(true);
    expect(record.closure.hazards.map(hazard => hazard.kind)).toEqual(
      expect.arrayContaining(["nonliteral-dynamic-loading", "eval"])
    );
    expect(record.exports.value).toBeDefined();
  });

  test("binds external re-exports only through accepted dependency export identities", () => {
    const dependencyRoot = fixture(
      { name: "accepted", version: "1.0.0" },
      {
        "index.js": "export const child = 1; export const renamed = 2;\n",
        "index.d.ts": "export declare const child: number; export declare const renamed: number;\n"
      }
    );
    const root = fixture(
      {
        name: "wrapper",
        version: "1.0.0",
        exports: { ".": { types: "./index.d.ts", import: "./index.js" } }
      },
      {
        "index.js":
          'import { renamed as imported } from "accepted"; export * from "accepted"; export { renamed as alias } from "accepted"; export { imported as importedAlias }; export const local = 3;\n',
        "index.d.ts":
          'import { renamed as imported } from "accepted"; export * from "accepted"; export { renamed as alias } from "accepted"; export { imported as importedAlias }; export declare const local: number;\n'
      }
    );
    const target = (axis, name) => ({
      module: {
        path: join(dependencyRoot, axis === "runtime" ? "index.js" : "index.d.ts"),
        digest: `sha256:${(axis === "runtime" ? "2" : "3").repeat(64)}`
      },
      exportName: name
    });
    const exports = Object.fromEntries(
      ["child", "renamed", "default"].map(name => [
        name,
        {
          runtime: target("runtime", name),
          declarations: target("declarations", name)
        }
      ])
    );

    const record = resolvePackageArtifacts({
      importer: join(root, "consumer.mjs"),
      specifier: "wrapper",
      packageRoot: root,
      integrity: "sha512:test",
      acceptedDependencies: {
        accepted: {
          packageName: "accepted",
          artifactCase: "artifact-case:accepted",
          acceptedContractDigest: `sha256:${"1".repeat(64)}`,
          exports
        }
      }
    });

    expect(Object.keys(record.exports)).toEqual([
      "alias",
      "child",
      "importedAlias",
      "local",
      "renamed"
    ]);
    expect(record.exports.child).toEqual(exports.child);
    expect(record.exports.alias).toEqual(exports.renamed);
    expect(record.exports.importedAlias).toEqual(exports.renamed);
    expect(record.exports.default).toBeUndefined();
  });

  test("names the missing accepted dependency for an import-then-export binding", () => {
    const root = fixture(
      {
        name: "wrapper",
        version: "1.0.0",
        exports: { ".": { types: "./index.d.ts", import: "./index.js" } }
      },
      {
        "index.js":
          'import { useContext as useDisclosureContext } from "@corvu/disclosure"; export { useDisclosureContext };\n',
        "index.d.ts":
          'export { useContext as useDisclosureContext } from "@corvu/disclosure";\n'
      }
    );

    expect(() => resolvePackageArtifacts({
      importer: join(root, "consumer.mjs"),
      specifier: "wrapper",
      packageRoot: root,
      integrity: "sha512:test"
    })).toThrow(/accepted dependency @corvu\/disclosure.*useContext/);

    const planned = resolvePackageArtifactClosure({
      importer: join(root, "consumer.mjs"),
      specifier: "wrapper",
      packageRoot: root,
      integrity: "sha512:test"
    });
    expect(planned.externalDependencies).toContainEqual({
      axis: "runtime",
      importerPath: "./index.js",
      kind: "reexport",
      specifier: "@corvu/disclosure"
    });
  });

  test("declaration closure resolves extensionless source modules", () => {
    const manifest = {
      name: "extensionless-source",
      version: "1.0.0",
      exports: { ".": { types: "./src/index.ts", import: "./src/index.ts" } }
    };
    const root = fixture(manifest, {
      "src/index.ts": 'export { value } from "./array";\n',
      "src/array.ts": "export const value = 1;\n"
    });
    const record = resolvePackageArtifacts({
      importer: join(root, "consumer.mjs"),
      specifier: "extensionless-source",
      packageRoot: root,
      integrity: "sha512:test"
    });
    expect(record.closure.entries).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ role: "runtime", path: "./src/array.ts" }),
        expect.objectContaining({ role: "declaration", path: "./src/array.ts" })
      ])
    );
  });

  test("declaration closure maps explicit source suffixes to declaration files", () => {
    const manifest = {
      name: "declaration-source-suffix",
      version: "1.0.0",
      exports: { ".": { types: "./dist/index.d.ts", import: "./dist/index.js" } }
    };
    const root = fixture(manifest, {
      "dist/index.js": "export const value = 1;\n",
      "dist/index.d.ts": 'export { value } from "./main.ts";\n',
      "dist/main.d.ts": "export declare const value: 1;\n"
    });
    const record = resolvePackageArtifacts({
      importer: join(root, "consumer.mjs"),
      specifier: "declaration-source-suffix",
      packageRoot: root,
      integrity: "sha512:test"
    });
    expect(record.exports.value.declarations.module.path).toBe(
      join(root, "dist", "main.d.ts")
    );
    expect(record.closure.entries).toContainEqual(
      expect.objectContaining({ role: "declaration", path: "./dist/main.d.ts" })
    );
  });

  test("multi-dot module basenames still try supported source suffixes", () => {
    const manifest = {
      name: "multi-dot-source",
      version: "1.0.0",
      exports: { ".": { types: "./src/index.ts", import: "./src/index.ts" } }
    };
    const root = fixture(manifest, {
      "src/index.ts": 'export { HeadContent } from "./HeadContent.dev";\n',
      "src/HeadContent.dev.tsx": "export const HeadContent = 1;\n"
    });
    const record = resolvePackageArtifacts({
      importer: join(root, "consumer.mjs"),
      specifier: "multi-dot-source",
      packageRoot: root,
      integrity: "sha512:test"
    });
    expect(record.exports.HeadContent.runtime.module.path).toBe(
      join(root, "src", "HeadContent.dev.tsx")
    );
    expect(record.exports.HeadContent.declarations.module.path).toBe(
      join(root, "src", "HeadContent.dev.tsx")
    );
  });

  test("literal dynamic chunk role propagates through static local children", () => {
    const manifest = {
      name: "dynamic-role",
      version: "1.0.0",
      exports: { ".": { types: "./index.d.ts", import: "./index.js" } }
    };
    const root = fixture(manifest, {
      "index.js": 'import("./chunk.js"); export const value = 1;\n',
      "chunk.js": 'import "./leaf.js"; export const chunk = 1;\n',
      "leaf.js": "export const leaf = 1;\n",
      "index.d.ts": "export declare const value: 1;\n"
    });
    const record = resolvePackageArtifacts({
      importer: join(root, "consumer.mjs"),
      specifier: "dynamic-role",
      packageRoot: root,
      integrity: "sha512:test"
    });
    expect(record.closure.entries).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ role: "literal-dynamic-chunk", path: "./chunk.js" }),
        expect.objectContaining({ role: "literal-dynamic-chunk", path: "./leaf.js" })
      ])
    );
  });

  test("ambient library symbols cannot mask mutable unbound globals", () => {
    const manifest = {
      name: "ambient-write",
      version: "1.0.0",
      exports: { ".": { types: "./index.d.ts", import: "./index.js" } }
    };
    const source =
      "/* 😀 */ Promise = replacement; { let Promise = replacement; Promise = replacement; } export const value = 1;\n";
    const root = fixture(manifest, {
      "index.js": source,
      "index.d.ts": "export declare const value: 1;\n"
    });
    const record = resolvePackageArtifacts({
      importer: join(root, "consumer.mjs"),
      specifier: "ambient-write",
      packageRoot: root,
      integrity: "sha512:test"
    });
    const hazards = record.closure.hazards.filter(
      hazard => hazard.kind === "mutable-unbound-global"
    );
    expect(hazards).toHaveLength(1);
    const expression = "Promise = replacement";
    const start = source.indexOf(expression);
    expect(hazards[0].source).toBe(
      `./index.js:${Buffer.byteLength(source.slice(0, start), "utf8")}-${
        Buffer.byteLength(source.slice(0, start + expression.length), "utf8")
      }`
    );
    const planned = resolvePackageArtifactClosure({
      importer: join(root, "consumer.mjs"),
      specifier: "ambient-write",
      packageRoot: root,
      integrity: "sha512:test"
    });
    expect(planned.closure.digest).toBe(record.closure.digest);
  });

  test("classifies native, WASM, and only genuinely unbound mutable globals", () => {
    const manifest = {
      name: "opaque-frontier",
      version: "1.0.0",
      exports: { ".": { types: "./index.d.ts", import: "./index.js" } }
    };
    const root = fixture(manifest, {
      "index.js":
        'require("./addon.node"); import("./module.wasm"); let local = 0; local = 1; local++; escaped = 1; advanced++; ({ destructured } = source); for (iterated of values) {} { let destructured = 0, iterated = 0; ({ destructured } = source); for (iterated of values) {} } export const value = local;\n',
      "index.d.ts": "export declare const value: number;\n"
    });
    const record = resolvePackageArtifacts({
      importer: join(root, "consumer.mjs"),
      specifier: "opaque-frontier",
      packageRoot: root,
      integrity: "sha512:test"
    });
    const kinds = record.closure.hazards.map(hazard => hazard.kind);
    expect(kinds).toEqual(
      expect.arrayContaining(["native-code", "opaque-wasm", "mutable-unbound-global"])
    );
    expect(kinds.filter(kind => kind === "mutable-unbound-global")).toHaveLength(4);
  });

  test("scope-resolves require, eval, and WebAssembly before opening a frontier", () => {
    const manifest = {
      name: "scope-frontier",
      version: "1.0.0",
      exports: { ".": { types: "./index.d.ts", import: "./index.js" } }
    };
    const root = fixture(manifest, {
      "index.js": `
        require(dynamicName);
        function local(require, eval) {
          require("./not-a-module.js");
          eval("not-global-eval");
          const WebAssembly = { instantiate() {} };
          WebAssembly.instantiate(bytes);
        }
        export const value = 1;
      `,
      "index.d.ts": "export declare const value: number;\n"
    });
    const record = resolvePackageArtifacts({
      importer: join(root, "consumer.mjs"),
      specifier: "scope-frontier",
      packageRoot: root,
      integrity: "sha512:test"
    });
    const kinds = record.closure.hazards.map(hazard => hazard.kind);
    expect(kinds.filter(kind => kind === "nonliteral-dynamic-loading")).toHaveLength(1);
    expect(kinds).not.toContain("eval");
    expect(kinds).not.toContain("opaque-wasm");
  });

  test("same bytes under a different closure path have different digests", () => {
    const make = name => {
      const manifest = {
        name,
        version: "1.0.0",
        exports: { ".": { types: "./index.d.ts", import: "./index.js" } }
      };
      return fixture(manifest, {
        "index.js": 'export { value } from "./leaf.js";\n',
        "leaf.js": "export const value = 1;\n",
        "index.d.ts": 'export { value } from "./leaf.d.ts";\n',
        "leaf.d.ts": "export declare const value: 1;\n"
      });
    };
    const firstRoot = make("first");
    const secondRoot = make("second");
    writeFileSync(join(secondRoot, "index.js"), 'export { value } from "./nested/leaf.js";\n');
    mkdirSync(join(secondRoot, "nested"));
    writeFileSync(join(secondRoot, "nested/leaf.js"), "export const value = 1;\n");
    const first = resolvePackageArtifacts({
      importer: join(firstRoot, "consumer.mjs"),
      specifier: "first",
      packageRoot: firstRoot,
      integrity: "sha512:first"
    });
    const second = resolvePackageArtifacts({
      importer: join(secondRoot, "consumer.mjs"),
      specifier: "second",
      packageRoot: secondRoot,
      integrity: "sha512:second"
    });
    expect(first.closure.digest).not.toBe(second.closure.digest);
  });

  test("preserves logical and real roots for symlinked nested installs", () => {
    const realRoot = fixture(
      {
        name: "linked",
        version: "1.0.0",
        exports: { ".": { types: "./index.d.ts", import: "./index.js" } }
      },
      {
        "index.js": "export const value = 1;\n",
        "index.d.ts": "export declare const value: 1;\n"
      }
    );
    const project = mkdtempSync(join(tmpdir(), "solid-checker-phase7-project-"));
    roots.push(project);
    const logical = join(project, "node_modules/linked");
    mkdirSync(dirname(logical), { recursive: true });
    symlinkSync(realRoot, logical, "dir");
    const record = resolvePackageArtifacts({
      importer: join(project, "src/app.mjs"),
      specifier: "linked",
      integrity: "sha512:linked"
    });
    expect(record.packageRoot).toBe(logical);
    const canonicalRoot = realpathSync(realRoot);
    expect(record.packageRealRoot).toBe(canonicalRoot);
    expect(record.runtime.realPath).toBe(join(canonicalRoot, "index.js"));
  });

  test("materialized generated output is hash-bound to bytes and transform identity", () => {
    const first = materializedGeneratedClosureEntry({
      stableId: "virtual-server",
      bytes: Buffer.from("first"),
      transformDigest: `sha256:${"2".repeat(64)}`
    });
    const second = materializedGeneratedClosureEntry({
      stableId: "virtual-server",
      bytes: Buffer.from("second"),
      transformDigest: `sha256:${"2".repeat(64)}`
    });
    expect(first.digest).not.toBe(second.digest);
    expect(() => materializedGeneratedClosureEntry({ stableId: "missing", bytes: Buffer.of() })).toThrow(
      /requires stable bytes/
    );
    const sharedGolden = materializedGeneratedClosureEntry({
      stableId: "server-function:entry",
      bytes: Buffer.from("first"),
      transformDigest: `sha256:${"2".repeat(64)}`
    });
    expect(canonicalClosure([sharedGolden]).digest).toBe(
      "sha256:fc5aa068103c10e1e89193af111113541668254cd82440497dbe8cb72e48f961"
    );
  });
});

test("accepts direct and default-wrapped TypeScript CommonJS namespaces", () => {
  const api = {
    ScriptTarget: { Latest: 99 },
    createProgram() {}
  };

  expect(selectTypeScriptApi(api)).toBe(api);
  expect(selectTypeScriptApi({ default: api })).toBe(api);
});

test("loads the TypeScript compiler API from a repository-root Bun process", () => {
  const repositoryRoot = join(import.meta.dirname, "../../..");
  const result = spawnSync(
    process.execPath,
    ["--eval", 'await import("./packages/cli/scripts/artifact-resolution.mjs")'],
    { cwd: repositoryRoot, encoding: "utf8" }
  );

  expect(result.status, result.stderr).toBe(0);
});

test("the standard condition vocabulary is one shared list", () => {
  for (const standard of [
    "default",
    "types",
    "import",
    "require",
    "node-addons",
    "browser",
    "node",
    "deno",
    "worker",
    "development",
    "production",
    "csr",
    "string-ssr",
    "streaming-ssr",
    // This checker's own ecosystem default: vite-plugin-solid and solid-start
    // activate it unconditionally, so it is not a private opt-in name.
    "solid"
  ]) {
    expect(isCustomCondition(standard), standard).toBe(false);
  }
  for (const custom of ["@solid-devtools/source", "vendor/source", "bun"]) {
    expect(isCustomCondition(custom), custom).toBe(true);
  }
});

test("only a namespaced custom condition is private to one consumer", () => {
  for (const namespaced of [
    "@solid-devtools/source",
    "vendor/source",
    "@scope/whatever",
    "tool/dev"
  ]) {
    expect(isPrivateNamespacedCondition(namespaced), namespaced).toBe(true);
  }
  // Bare-name custom conditions are ecosystem-activated, never private: each of
  // these is switched on unconditionally by the runtime or bundler that owns it,
  // so a missing target behind one is a defective publish real consumers hit.
  for (const bare of [
    "bun",
    "workerd",
    "edge-light",
    "react-native",
    "electron",
    "svelte",
    "solid",
    "browser",
    "default"
  ]) {
    expect(isPrivateNamespacedCondition(bare), bare).toBe(false);
  }
});

test("non-module target extensions are exactly the non-executable resource list", () => {
  for (const module of [
    "./a.js",
    "./a.mjs",
    "./a.cjs",
    "./a.jsx",
    "./a.ts",
    "./a.mts",
    "./a.cts",
    "./a.tsx"
  ]) {
    expect(nonModuleTargetExtension(module), module).toBeUndefined();
  }
  expect(nonModuleTargetExtension("./a.js.map")).toBe(".map");
  expect(nonModuleTargetExtension("./a.jsx.map")).toBe(".map");
  expect(nonModuleTargetExtension("./a.module.css")).toBe(".css");
  expect(nonModuleTargetExtension("./a.json")).toBe(".json");
  expect(nonModuleTargetExtension("./a.png")).toBe(".png");
  expect(nonModuleTargetExtension("./a.woff2")).toBe(".woff2");
  expect(nonModuleTargetExtension("./a.svg")).toBe(".svg");
  expect(nonModuleTargetExtension("./README.md")).toBe(".md");
  // `.node` and `.wasm` are executable entrypoints this pipeline cannot read.
  // The closure already names them native-code/opaque-wasm hazards, so they are
  // emphatically not "nothing to assert" — they keep certify-or-refuse.
  expect(nonModuleTargetExtension("./build/addon.node")).toBeUndefined();
  expect(nonModuleTargetExtension("./build/engine.wasm")).toBeUndefined();
  // An unknown extension is likewise not answered here: the list is positive,
  // so a shape nothing models refuses instead of silently asserting nothing.
  expect(nonModuleTargetExtension("./a.json5")).toBeUndefined();
  expect(nonModuleTargetExtension("./a.coffee")).toBeUndefined();
  // A declaration file ends in a module extension on purpose: whether a `.d.ts`
  // runtime target is analyzable is a separate question this rule must not
  // silently answer.
  expect(nonModuleTargetExtension("./a.d.ts")).toBeUndefined();
  // No extension is not answered here.
  expect(nonModuleTargetExtension("./bin")).toBeUndefined();
});

test("export target selection reports the conditions traversed and target presence", () => {
  const root = fixture(
    {
      name: "selection",
      version: "1.0.0",
      exports: {
        ".": { "vendor/source": "./src/index.ts", browser: "./browser.js", import: "./index.js" }
      }
    },
    { "index.js": "export const value = 1;\n", "browser.js": "export const value = 2;\n" }
  );
  const manifest = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
  const select = conditions =>
    selectPackageExportTarget({ packageRoot: root, manifest, entrypoint: ".", conditions });

  expect(select([]).conditions).toEqual(["import"]);
  expect(select([]).exists).toBe(true);
  expect(select(["browser"]).conditions).toEqual(["browser"]);
  expect(select(["browser"]).exists).toBe(true);
  const custom = select(["vendor/source"]);
  expect(custom.conditions).toEqual(["vendor/source"]);
  expect(custom.exists).toBe(false);
  expect(custom.path.endsWith("src/index.ts")).toBe(true);
  // The persisted trace is unchanged: the condition list is a separate field so
  // the resolution record's digest does not move.
  expect(custom.trace.steps.map(step => step.condition)).toEqual([
    "subpath",
    "vendor/source",
    "target"
  ]);
});
