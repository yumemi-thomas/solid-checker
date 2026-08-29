import { afterEach, describe, expect, test } from "vitest";
import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  realpathSync,
  rmSync,
  symlinkSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

import {
  ArtifactResolutionError,
  canonicalClosure,
  materializedGeneratedClosureEntry,
  resolvePackageArtifacts,
  resolvePackageExport,
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
});

describe("exact artifact records and closure", () => {
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
