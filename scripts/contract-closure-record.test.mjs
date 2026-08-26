// Closure-record properties that only the *real* producer can establish, in
// package shapes no checked-in fixture can carry.
//
// Everything here turns on which path spelling the analyzing program answers
// with, and on the filesystem underneath it. A stub cannot produce those: it has
// no resolver, so scripts/contract-generation.test.mjs drives the reconciliation
// branches with declared import facts instead. And the corpus cannot carry these
// packages either -- one needs a symlink that escapes the package root, which
// would be the repository's first committed symlink and would arrive as a plain
// file on a Windows checkout.
//
// So they are generated into a temporary directory here, against the pinned
// checker and producer, and skip cleanly when either binary is absent.
//
// The one claim every case shares: a module the analyzing program opened is
// either *in* the record or *named* in a note. It is never dropped.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { test } from "vitest";

const root = resolve(import.meta.dirname, "..");
const cli = join(root, "packages/cli/bin/solid-checker.mjs");
const native =
  process.env.SOLID_CHECKER_NATIVE_BIN ?? join(root, "rust/target/debug/solid-checker-rust");
const typeFacts = process.env.SOLID_TYPEFACTS_BIN ?? join(root, "bin/solid-typefacts");
const canRun = existsSync(native) && existsSync(typeFacts);

/// One package generated with the real engine, and the closure record it wrote.
///
/// `files` is written verbatim under the package root; `prepare` runs before
/// generation for a shape a file map cannot express.
function record(name, { manifest, files, prepare } = {}) {
  const directory = mkdtempSync(join(tmpdir(), `solid-checker-closure-${name}-`));
  const packageRoot = join(directory, "package");
  mkdirSync(packageRoot, { recursive: true });
  writeFileSync(
    join(packageRoot, "package.json"),
    `${JSON.stringify(
      { name, version: "1.0.0", type: "module", exports: { ".": "./index.js" }, ...manifest },
      null,
      2
    )}\n`
  );
  for (const [path, contents] of Object.entries(files ?? {})) {
    const file = join(packageRoot, path);
    mkdirSync(join(file, ".."), { recursive: true });
    writeFileSync(file, contents);
  }
  prepare?.({ directory, packageRoot });
  // Beside the manifest, so the record's paths are package-relative and the
  // assertions below can name them the way a fixture's own pin does.
  const output = join(packageRoot, "solid-reactivity.json");
  const result = spawnSync(
    process.execPath,
    [cli, "contract", "generate", "--package-root", packageRoot, "--output", output],
    {
      cwd: root,
      env: {
        ...process.env,
        SOLID_CHECKER_NATIVE_BIN: native,
        SOLID_TYPEFACTS_BIN: typeFacts
      },
      encoding: "utf8"
    }
  );
  assert.equal(result.status, 0, result.stderr);
  const plan = JSON.parse(readFileSync(join(packageRoot, "solid-reactivity.review.json"), "utf8"));
  return { directory, packageRoot, closure: plan.generation.entrypoints["."] };
}

test("a symlinked directory inside the package stays in the record", { skip: !canRun }, () => {
  // `src -> ../shared`: the module is this package's own file by every spelling
  // a user would recognize, and its realpath is outside the package root.
  // TypeScript takes a realpath only where resolution walked a symlink under
  // `node_modules`, so the analyzing program answers with the spelled path --
  // and a record that canonicalized first and filtered second dropped the module
  // from the hash set *and* from both reconciliation sweeps, leaving a record
  // that read as a complete attestation while the file every summary came from
  // went unnamed. That is the defect class the attested record exists to close,
  // so it must not be reachable through the record's own filter.
  const { directory, closure } = record("symlinked-directory", {
    files: { "index.js": 'export { helper } from "./src/impl.js";\nexport const thing = 1;\n' },
    prepare: ({ directory: base, packageRoot }) => {
      mkdirSync(join(base, "shared"), { recursive: true });
      writeFileSync(join(base, "shared", "impl.js"), "export const helper = 42;\n");
      symlinkSync(join("..", "shared"), join(packageRoot, "src"));
    }
  });
  try {
    assert.deepEqual(closure.modules.map(module => module.path).sort(), [
      "index.js",
      "src/impl.js"
    ]);
    for (const module of closure.modules) assert.match(module.hash, /^sha256:[0-9a-f]{64}$/);
    // Recorded, so there is nothing left to note: the omission is gone rather
    // than merely reported.
    assert.equal(closure.notes, undefined);
    assert.equal(closure.runtimeNotes, undefined);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("re-exporting through an unselected branch refuses the entrypoint", { skip: !canRun }, () => {
  // The control for fixtures/package-contracts/conditional-imports-side-effect,
  // and the measurement of how far that hole reached. The same unresolvable
  // `#platform` specifier, read for a *value* instead of for its side effect:
  // the re-exported binding's runtime kind no closed type answers, so the
  // analyzer refuses the entrypoint and the package has nothing to certify.
  //
  // That is what makes the side-effect import the only shape that could certify
  // silently -- an import whose failure to resolve does not propagate into any
  // export summary. It cannot be a corpus fixture, because generation exits
  // non-zero and writes no contract to pin.
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-closure-reexport-"));
  const packageRoot = join(directory, "package");
  mkdirSync(packageRoot, { recursive: true });
  try {
    writeFileSync(
      join(packageRoot, "package.json"),
      `${JSON.stringify({
        name: "conditional-imports-reexport",
        version: "1.0.0",
        type: "module",
        exports: { ".": "./index.js" },
        imports: { "#platform": { browser: "./browser.mjs", node: "./node.mjs" } }
      })}\n`
    );
    writeFileSync(
      join(packageRoot, "index.js"),
      'export { branch } from "#platform";\nexport const thing = 1;\n'
    );
    for (const branch of ["browser", "node"]) {
      writeFileSync(join(packageRoot, `${branch}.mjs`), `export const branch = "${branch}";\n`);
    }
    const result = spawnSync(
      process.execPath,
      [
        cli,
        "contract",
        "generate",
        "--package-root",
        packageRoot,
        "--output",
        join(packageRoot, "solid-reactivity.json")
      ],
      {
        cwd: root,
        env: { ...process.env, SOLID_CHECKER_NATIVE_BIN: native, SOLID_TYPEFACTS_BIN: typeFacts },
        encoding: "utf8"
      }
    );
    assert.notEqual(result.status, 0, result.stdout);
    assert.match(result.stderr, /has no certifiable runtime entrypoint/);
    assert.match(
      result.stderr,
      /"branch", whose runtime kind no closed type answers \(Unknown, Unknown\)/
    );
    assert.equal(existsSync(join(packageRoot, "solid-reactivity.json")), false);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a dependency's own bytes are not this package's record", { skip: !canRun }, () => {
  // The real resolver reading a real installed dependency, which is what the
  // stub suite cannot do. The analysis opens `node_modules/dep/index.d.ts`; the
  // record must not, because hashing it binds the record to the install layout
  // (hoisted and the file is elsewhere, nested and it is here) and to a
  // dependency's version, so two generations over byte-identical package bytes
  // would refuse to transfer a review.
  const { directory, closure } = record("nested-dependency", {
    manifest: { dependencies: { dep: "1.0.0" } },
    files: {
      "index.js": 'import { dep } from "dep";\nexport const thing = dep;\n',
      "node_modules/dep/package.json": JSON.stringify({
        name: "dep",
        version: "1.0.0",
        type: "module",
        main: "index.js",
        types: "index.d.ts"
      }),
      "node_modules/dep/index.js": "export const dep = 1;\n",
      "node_modules/dep/index.d.ts": "export declare const dep: number;\n"
    }
  });
  try {
    assert.deepEqual(closure.modules.map(module => module.path), ["index.js"]);
    assert.equal(closure.notes, undefined);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("one file reached by two spellings is one module", { skip: !canRun }, () => {
  // On a case-insensitive filesystem both specifiers resolve to `impl.js`, the
  // walk seeds two roots for one file, and the analyzing program answers with
  // whichever spelling it was handed: the record named `Impl.js`, which exists
  // on no case-sensitive filesystem, and the seed sweep reported `impl.js` as
  // seeded-but-never-opened -- a false note about a file that was read.
  //
  // The assertions hold on both kinds of filesystem, which is the property that
  // matters: a record is transferred between machines, so the verdict may not
  // depend on which one generated it. On a case-sensitive filesystem `./Impl.js`
  // resolves nowhere, names no existing runtime module, and so says nothing.
  const { directory, closure } = record("case-folded", {
    files: {
      "index.js": 'export { helper } from "./Impl.js";\nexport { other } from "./impl.js";\n',
      "impl.js": "export const helper = 1;\nexport const other = 2;\n"
    }
  });
  try {
    assert.deepEqual(closure.modules.map(module => module.path).sort(), ["impl.js", "index.js"]);
    assert.equal(closure.notes, undefined);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
