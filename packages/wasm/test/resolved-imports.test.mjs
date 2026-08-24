// The WASM entry point has no Type Facts session of its own -- the host runs
// TypeScript and hands the finished tables in -- so it cannot ask the compiler
// where an import specifier resolves. Package contracts are bound to the
// installed package a specifier resolves to, which makes that question part of
// the request.
//
// Both paths are pinned here, because either one silently changing is a defect:
// a request without `resolvedImports` keeps the older name-matched behavior
// exactly, and a request with it binds by installed identity and refuses a
// specifier the field does not cover.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

const require = createRequire(import.meta.url);
const { checkSync } = require("../node.cjs");

const SOURCE = [
  'import { mapValue } from "reactive-package";',
  "function named(index: number, item: () => number) {",
  "  return item();",
  "}",
  "export function use() {",
  "  mapValue(named);",
  "}",
  ""
].join("\n");

// One reviewed contract whose callback claim carries an argument descriptor a
// by-name callback cannot bind, so a bound contract raises exactly one SC9005
// and a refused one raises none.
const CONTRACT = {
  schemaVersion: 1,
  package: { name: "reactive-package", version: "1.0.0" },
  compilerFactsProtocol: 1,
  summaries: {
    "map-value": {
      kind: "function",
      callbacks: [
        {
          parameter: 0,
          execution: "inline",
          arguments: [null, { kind: "accessor", label: "item" }]
        }
      ]
    }
  },
  entrypoints: { ".": { exports: { "map-value": ["mapValue"] } } },
  evidence: { kind: "reviewed" }
};

function project() {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-wasm-identity-"));
  const installed = join(root, "node_modules/reactive-package");
  mkdirSync(installed, { recursive: true });
  writeFileSync(
    join(installed, "package.json"),
    JSON.stringify({ name: "reactive-package", version: "1.0.0", types: "index.d.ts" }) + "\n"
  );
  writeFileSync(
    join(installed, "index.d.ts"),
    "export declare function mapValue(\n  map: (index: number, item: () => number) => unknown\n): void;\n"
  );
  writeFileSync(join(installed, "solid-reactivity.json"), JSON.stringify(CONTRACT) + "\n");
  return root;
}

// The span of an identifier occurrence in SOURCE. The host's own TypeScript
// engine reports these; here they are computed from the one source so the test
// states no offset by hand.
function span(needle, occurrence = 1) {
  let index = -1;
  for (let n = 0; n < occurrence; n += 1) index = SOURCE.indexOf(needle, index + 1);
  return { start: index, end: index + needle.length };
}

function check(root, resolvedImports) {
  const projectId = join(root, "tsconfig.json");
  const path = join(root, "App.ts");
  const entity = ({ start, end }) => ({
    location: { path, startByte: start, endByte: end },
    symbol: "map-value-symbol"
  });
  const request = {
    projectId,
    generation: 1,
    sources: [{ path, source: SOURCE }],
    typeFacts: {
      schema: 2,
      generation: 1,
      projectId,
      sources: [
        { path, sha256: `sha256:${createHash("sha256").update(SOURCE).digest("hex")}` }
      ],
      entities: [entity(span("mapValue", 1)), entity(span("mapValue", 2))],
      symbols: [
        {
          id: "map-value-symbol",
          declarations: [],
          references: [
            { path, startByte: span("mapValue", 1).start, endByte: span("mapValue", 1).end },
            { path, startByte: span("mapValue", 2).start, endByte: span("mapValue", 2).end }
          ]
        }
      ],
      files: []
    }
  };
  if (resolvedImports !== undefined) request.resolvedImports = resolvedImports;
  return JSON.parse(checkSync(JSON.stringify(request)));
}

function specifier(path, extra) {
  const literal = '"reactive-package"';
  return {
    files: [
      {
        path,
        imports: [
          {
            startByte: SOURCE.indexOf(literal),
            endByte: SOURCE.indexOf(literal) + literal.length,
            text: "reactive-package",
            ...extra
          }
        ]
      }
    ]
  };
}

const contractFindings = snapshot =>
  (snapshot.findings ?? []).filter(finding => finding.id === "SC9005");

test("a request without resolvedImports keeps name-matched contracts", () => {
  const root = project();
  try {
    // Unchanged behavior, not a weaker analysis of the same request: this entry
    // point has always bound a discovered contract to the specifier's name, and
    // a host that cannot answer for its resolutions still gets that.
    assert.equal(contractFindings(check(root, undefined)).length, 1);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("resolvedImports binds a contract to the install the specifier resolves to", () => {
  const root = project();
  try {
    const path = join(root, "App.ts");
    const bound = check(
      root,
      specifier(path, {
        resolution: "nodeModules",
        resolvedPath: join(root, "node_modules/reactive-package/index.d.ts"),
        packageName: "reactive-package",
        resolverPackageName: "reactive-package"
      })
    );
    assert.equal(contractFindings(bound).length, 1);

    // The shadow shape: the package is installed and carries the contract, and
    // this specifier resolves to project source the contract never described.
    const refused = check(
      root,
      specifier(path, {
        resolution: "nonRelative",
        resolvedPath: join(root, "src/local-impl.ts"),
        packageName: "the-project"
      })
    );
    assert.deepEqual(contractFindings(refused), []);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("a specifier the field does not cover is refused, not name-matched", () => {
  const root = project();
  try {
    // Supplying the field is a claim that these are the resolutions. A file it
    // omits is unanswered, and an unanswered specifier certifies nothing --
    // falling back to the name here would make the field partially trusted.
    assert.deepEqual(contractFindings(check(root, { files: [] })), []);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("an unrecognized resolution is refused rather than guessed at", () => {
  const root = project();
  try {
    const path = join(root, "App.ts");
    assert.throws(
      () => check(root, specifier(path, { resolution: "probably-node-modules" })),
      /unknown module resolution/
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

// A host that forwards TypeScript's own positions without converting them
// reports UTF-16 code units where this field is documented in bytes. The two
// agree for ASCII and drift by the number of non-ASCII characters before the
// specifier, so the mistake is invisible in small files and silently stops
// binding in larger ones -- contract coverage varying by file, with no error.
// This is the one shape that must not be a refusal.
test("a specifier span that is not bytes into this source is an error", () => {
  // Long enough that the UTF-16 span lands outside the specifier entirely,
  // which is the case that used to silently refuse the contract.
  const source = `// ${"\u{1F600}".repeat(40)}\n${SOURCE}`;
  const bytes = text => Buffer.byteLength(text, "utf8");
  const literal = '"reactive-package"';
  const at = index => ({
    startByte: bytes(source.slice(0, index)),
    endByte: bytes(source.slice(0, index + literal.length))
  });
  const root = project();
  try {
    const path = join(root, "App.ts");
    const projectId = join(root, "tsconfig.json");
    const request = row => ({
      projectId,
      generation: 1,
      sources: [{ path, source }],
      typeFacts: {
        schema: 2,
        generation: 1,
        projectId,
        sources: [
          { path, sha256: `sha256:${createHash("sha256").update(source).digest("hex")}` }
        ],
        entities: [],
        symbols: [],
        files: []
      },
      resolvedImports: {
        files: [
          {
            path,
            imports: [
              {
                ...row,
                text: "reactive-package",
                resolution: "nodeModules",
                resolvedPath: join(root, "node_modules/reactive-package/index.d.ts"),
                packageName: "reactive-package"
              }
            ]
          }
        ]
      }
    });
    const utf16 = source.indexOf(literal);
    // The byte span is accepted; the UTF-16 span for the same specifier is not.
    assert.doesNotThrow(() => JSON.parse(checkSync(JSON.stringify(request(at(utf16))))));
    assert.throws(
      () =>
        checkSync(
          JSON.stringify(request({ startByte: utf16, endByte: utf16 + literal.length }))
        ),
      /offsets are UTF-8 bytes, not UTF-16 code units/
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

// `unresolved` is *accepted* by contract binding -- nothing resolved means
// nothing else claimed the specifier -- so a host that labels resolutions it
// did not perform would get every contract applied. It is the only mistake in
// this interface that fails open, and the invariant it violates is checkable.
test("a resolution that disagrees with its resolvedPath is an error", () => {
  const root = project();
  try {
    const path = join(root, "App.ts");
    assert.throws(
      () =>
        check(
          root,
          specifier(path, { resolution: "unresolved", resolvedPath: join(root, "src/local.ts") })
        ),
      /empty exactly when the resolution is "unresolved"/
    );
    assert.throws(
      () => check(root, specifier(path, { resolution: "nodeModules" })),
      /empty exactly when the resolution is "unresolved"/
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("documents that a request without the field binds contracts by name", () => {
  const readme = require("node:fs").readFileSync(new URL("../README.md", import.meta.url), "utf8");
  const declarations = require("node:fs").readFileSync(
    new URL("../index.d.ts", import.meta.url),
    "utf8"
  );
  assert.match(readme, /resolvedImports/);
  assert.match(readme, /binds package contracts by specifier name/);
  assert.match(declarations, /resolvedImports\?/);
});
