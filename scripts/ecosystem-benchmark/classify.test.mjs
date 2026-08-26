import assert from "node:assert/strict";
import { test } from "vitest";

import { FAILURE_CLASSES, classifyResult, normalizeSignature } from "./lib/classify.mjs";

// Three real captured lines (see task briefing) — treated as ground truth for
// what this checker actually emits.
const DISPATCH_LINE_ROUTER =
  'solid-checker: solid-checker-rust: emit package contract: unresolved obligation at /tmp/x/node_modules/@solidjs/router/dist/data/action.js:1364: ReactiveDispatchUnresolved { callee: "action", member: Some("toString") }';
const TYPE_FACTS_LINE =
  "solid-checker: solid-checker-rust: native Solid compiler facts error: /tmp/x/node_modules/@kobalte/core/dist/chunk/DOJAEHTL.jsx: semantic trace has unresolved execution sites: NativeAttribute@3275..3298";
const DISPATCH_LINE_DEVTOOLS =
  'solid-checker: solid-checker-rust: emit package contract: unresolved obligation at /tmp/x/node_modules/solid-devtools/dist/chunk-RDZMZMK7.js:3898: ReactiveDispatchUnresolved { callee: "getTarget", member: Some("has") }';
const SUCCESS_LINE =
  "generated @solid-primitives/scheduled@1.5.0 contract with 1 entrypoints at /tmp/out/scheduled.json; review plan /tmp/out/scheduled.review.md (12 checklist items)";
// The same line the generator writes when it refused entrypoints: exit 0, a
// contract on disk, and an explicit note naming how many entrypoints it does
// NOT describe.
const PARTIAL_SUCCESS_LINE =
  "generated @kobalte/core@0.13.13 contract with 28 entrypoints at /tmp/out/kobalte.json; 16 entrypoint(s) refused and omitted; review plan /tmp/out/kobalte.review.md (91 checklist items)";
const EXPORT_KIND_UNRESOLVED_LINE =
  'solid-checker: @solid-primitives/platform has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "isApple", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it invokes no caller-supplied callback';
const EXPORT_KIND_CONFLICT_LINE =
  "solid-checker: package contract value export .:createGeolocation cannot have function effects";

// Real captured lines from live `contract generate` runs against
// @tanstack/solid-query, corvu, @solidjs/meta, and @solid-primitives/map (see
// the scratchpad's p3/*/gen.log fixtures). These pin the three normalizeSignature
// defects reported after the initial implementation: scoped package/module
// names were only half-erased (the scope survived), and the
// unresolved-parameter-behavior signature swallowed an entire schema-v1 JSON
// stub.
const EXPORT_ALL_TANSTACK =
  'solid-checker: solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/query-core" from /private/tmp/claude-501/-Users-thomas-Documents-Github-solid-checker/3ecbd094-f261-4184-87e6-30b4ed4fd56e/scratchpad/p3/_tanstack_solid-query/node_modules/@tanstack/solid-query/src/index.ts; generate and pass its dependency contract with --contract';
const EXPORT_ALL_CORVU =
  'solid-checker: solid-checker-rust: emit package contract: cannot statically expand external export-all "@corvu/accordion" from /private/tmp/claude-501/-Users-thomas-Documents-Github-solid-checker/3ecbd094-f261-4184-87e6-30b4ed4fd56e/scratchpad/p3/corvu/node_modules/corvu/dist/accordion.jsx; generate and pass its dependency contract with --contract';
const CONDITIONAL_META_LINE =
  'solid-checker: @solidjs/meta .:Stylesheet has different semantics across overlapping conditional-export branches [] and ["solid"]; schema v1 cannot represent export-map fallback ordering, so split the entrypoint or review an explicit contract';
const PARAMETER_BEHAVIOR_MAP_LINE =
  'solid-checker: solid-checker-rust: emit package contract: unresolved parameter behavior in createMap parameter 0 (any) at /private/tmp/claude-501/-Users-thomas-Documents-Github-solid-checker/3ecbd094-f261-4184-87e6-30b4ed4fd56e/scratchpad/p3/_solid-primitives_map/node_modules/@solid-primitives/map/dist/index.js:5783: parameter 0 (any) is passed to resolved ReactiveMap.constructor from /private/tmp/claude-501/-Users-thomas-Documents-Github-solid-checker/3ecbd094-f261-4184-87e6-30b4ed4fd56e/scratchpad/p3/_solid-primitives_map/node_modules/@solid-primitives/map/dist/index.js, but no package contract proves when it executes; generate or add the exact package entrypoint/export contract before this callback can be certified; required behavior: choose exactly one audited mode: inline, tracked, or deferred; edit this schema-v1 stub and review its evidence: {"compilerFactsProtocol":1,"entrypoints":{".":{"exports":{"callback-stub":["createMap"]}}},"evidence":{"generator":"solid-checker unknown-callback","kind":"<set reviewed after auditing runtime behavior>"},"package":{"name":"current project","version":"<exact-installed-version>"},"schemaVersion":1,"summaries":{"callback-stub":{"callbacks":[{"execution":"<choose: inline | tracked | deferred>","parameter":0}],"kind":"function"}}}}';

test("FAILURE_CLASSES matches the documented exact id list, in order", () => {
  assert.deepEqual(FAILURE_CLASSES, [
    "success",
    "partial-success",
    "unsupported-package-shape",
    "no-esm-runtime-target",
    "no-exported-surface",
    "cjs-only-entrypoint",
    "conditional-export-incompatible",
    "incompatible-conditional-summaries",
    "unresolved-parameter-behavior",
    "reactive-dispatch-unresolved",
    "reactive-source-uncaptured",
    "dependency-contract-obligation",
    "package-contract-environment-dependent",
    "package-contract-export-missing",
    "export-kind-unresolved",
    "export-kind-conflict",
    "type-facts-failure",
    "checker-crash",
    "timeout",
    "install-failure",
    "integrity-failure",
    "unclassified"
  ]);
});

test("every failure class is reachable and each documented stderr shape maps to its class", () => {
  const cases = [
    {
      class: "success",
      result: classifyResult({ status: 0, stdout: SUCCESS_LINE, stderr: "", phase: "generate" })
    },
    {
      class: "partial-success",
      result: classifyResult({ status: 0, stdout: PARTIAL_SUCCESS_LINE, stderr: "", phase: "generate" })
    },
    {
      class: "unsupported-package-shape",
      result: classifyResult({
        status: 1,
        stdout: "",
        stderr: "/tmp/x/node_modules/pkg/package.json must declare name and version for package contract generation",
        phase: "generate"
      })
    },
    {
      // A resolved, parseable ESM entry that simply exports nothing is not the
      // same finding as a package whose declared runtime target is unusable.
      class: "no-exported-surface",
      result: classifyResult({
        status: 1,
        stdout: "",
        stderr: "emit package contract: entry file /tmp/x/index.js has no runtime ESM exports",
        phase: "generate"
      })
    },
    {
      class: "no-esm-runtime-target",
      result: classifyResult({
        status: 1,
        stdout: "",
        stderr: "solid-checker: pkg has no supported ESM runtime entrypoints",
        phase: "generate"
      })
    },
    {
      class: "cjs-only-entrypoint",
      result: classifyResult({
        status: 1,
        stdout: "",
        stderr: "./dist/index.cjs has only a CJS runtime target; CJS contract generation is unsupported",
        phase: "generate"
      })
    },
    {
      class: "conditional-export-incompatible",
      result: classifyResult({
        status: 1,
        stdout: "",
        stderr:
          'pkg ./dist/index.js:createFoo has different semantics across overlapping conditional-export branches "worker" and "browser"; schema v1 cannot represent...',
        phase: "generate"
      })
    },
    {
      class: "incompatible-conditional-summaries",
      result: classifyResult({
        status: 1,
        stdout: "",
        stderr: 'pkg ./dist/index.js:createFoo has incompatible semantics across conditional targets: "worker" versus "browser"',
        phase: "generate"
      })
    },
    {
      class: "unresolved-parameter-behavior",
      result: classifyResult({
        status: 1,
        stdout: "",
        stderr:
          'emit package contract: unresolved parameter behavior in createFoo parameter 0 (() => void) at /tmp/x/node_modules/pkg/index.js:12: cannot resolve execution; required behavior: sync; edit this schema-v1 stub and review its evidence: /tmp/out/pkg.review.md',
        phase: "generate"
      })
    },
    {
      class: "reactive-dispatch-unresolved",
      result: classifyResult({ status: 1, stdout: "", stderr: DISPATCH_LINE_ROUTER, phase: "generate" })
    },
    {
      class: "reactive-source-uncaptured",
      result: classifyResult({
        status: 1,
        stdout: "",
        stderr:
          'emit package contract: unresolved obligation at /tmp/x/node_modules/pkg/index.js:42: ReactiveSourceUncaptured { source: "props.value", callee: "createFoo" }',
        phase: "generate"
      })
    },
    {
      class: "dependency-contract-obligation",
      result: classifyResult({
        status: 1,
        stdout: "",
        stderr:
          'emit package contract: cannot statically expand external export-all "./sub" from /tmp/x/node_modules/pkg/index.js; generate and pass its dependency contract with --contract',
        phase: "generate"
      })
    },
    {
      class: "export-kind-unresolved",
      result: classifyResult({
        status: 2,
        stdout: "",
        stderr: EXPORT_KIND_UNRESOLVED_LINE,
        phase: "generate"
      })
    },
    {
      class: "export-kind-conflict",
      result: classifyResult({
        status: 2,
        stdout: "",
        stderr: EXPORT_KIND_CONFLICT_LINE,
        phase: "generate"
      })
    },
    {
      class: "type-facts-failure",
      result: classifyResult({ status: 1, stdout: "", stderr: TYPE_FACTS_LINE, phase: "generate" })
    },
    {
      class: "checker-crash",
      result: classifyResult({
        status: 134,
        stdout: "",
        stderr: "thread 'main' panicked at 'assertion failed', src/lib.rs:42:5",
        phase: "generate"
      })
    },
    {
      class: "timeout",
      result: classifyResult({ status: null, stdout: "", stderr: "", timedOut: true, phase: "generate" })
    },
    {
      class: "install-failure",
      result: classifyResult({
        status: 1,
        stdout: "",
        stderr: "npm error code E404\nnpm error 404 Not Found",
        phase: "install"
      })
    },
    {
      class: "integrity-failure",
      result: classifyResult({
        status: 1,
        stdout: "",
        stderr: "npm error code EINTEGRITY\nnpm error Verification failed while extracting pkg@1.0.0",
        phase: "install"
      })
    },
    // Both captured verbatim from the first full ecosystem run, where together
    // they accounted for 83 of the 84 `unclassified` results.
    {
      class: "package-contract-environment-dependent",
      result: classifyResult({
        status: 2,
        stdout: "",
        stderr:
          'solid-checker: solid-checker-rust: emit package contract: unresolved obligation at /tmp/x/node_modules/@kobalte/core/dist/accordion/CxG9lD9Q.js:646: PackageContractEnvironmentDependent { module: "@solidjs/web", export: "createComponent", reexported: false }',
        phase: "generate"
      })
    },
    {
      class: "package-contract-export-missing",
      result: classifyResult({
        status: 2,
        stdout: "",
        stderr:
          'solid-checker-rust: emit package contract: unresolved obligation at /tmp/x/node_modules/@tanstack/solid-router/dist/source/ClientOnly.jsx:1507: PackageContractExportMissing { module: "solid-js", export: "context", reexported: false }',
        phase: "generate"
      })
    }
  ];

  for (const { class: expectedClass, result } of cases) {
    assert.equal(result.class, expectedClass, `expected class ${expectedClass}, got ${result.class}`);
  }

  // Every id in FAILURE_CLASSES was actually produced above, plus `unclassified`
  // is covered by a dedicated test below.
  const produced = new Set(cases.map(c => c.class));
  for (const id of FAILURE_CLASSES) {
    if (id === "unclassified") continue;
    assert.ok(produced.has(id), `no case reached class ${id}`);
  }
});

test("ordering: the most specific marker wins when a message contains two markers", () => {
  // The "required behavior" tail of a real unresolved-parameter-behavior line
  // can itself name a reactive-dispatch enum variant. The specific
  // "unresolved parameter behavior" phrase must win over the generic
  // "ReactiveDispatchUnresolved" token embedded later in the same message.
  const stderr =
    'emit package contract: unresolved parameter behavior in createFoo parameter 0 (() => void) at /tmp/x/node_modules/pkg/index.js:12: cannot resolve execution; required behavior: ReactiveDispatchUnresolved { callee: "createFoo", member: None }; edit this schema-v1 stub and review its evidence: /tmp/out/pkg.review.md';
  const result = classifyResult({ status: 1, stdout: "", stderr, phase: "generate" });
  assert.equal(result.class, "unresolved-parameter-behavior");
  assert.notEqual(result.class, "reactive-dispatch-unresolved");
});

test("unrecognized stderr yields unclassified and retains the complete raw stderr byte-for-byte", () => {
  const weirdStderr = "totally unrecognized failure text\nwith\tmultiple\nlines and éè unicode";
  const result = classifyResult({ status: 1, stdout: "", stderr: weirdStderr, phase: "generate" });
  assert.equal(result.class, "unclassified");
  assert.equal(result.raw.stderr, weirdStderr);
  assert.equal(Buffer.byteLength(result.raw.stderr, "utf8"), Buffer.byteLength(weirdStderr, "utf8"));
});

test("a non-zero exit is never classified as success even when stdout contains the success wording", () => {
  const stdout =
    "generated pkg@1.0.0 contract with 3 entrypoints at /tmp/out/pkg.json; review plan /tmp/out/pkg.review.md (4 checklist items)";
  const result = classifyResult({ status: 1, stdout, stderr: "some unrelated error", phase: "generate" });
  assert.notEqual(result.class, "success");
});

test("detail extraction for the ReactiveDispatchUnresolved line", () => {
  const result = classifyResult({ status: 1, stdout: "", stderr: DISPATCH_LINE_ROUTER, phase: "generate" });
  assert.equal(result.class, "reactive-dispatch-unresolved");
  assert.equal(result.detail.defect, "ReactiveDispatchUnresolved");
  assert.equal(result.detail.callee, "action");
  assert.equal(result.detail.member, "toString");
  assert.equal(result.detail.offset, 1364);
  assert.equal(result.detail.dependency, "@solidjs/router");
});

test("normalizeSignature groups the two real ReactiveDispatchUnresolved lines despite different packages, paths, and offsets", () => {
  const a = normalizeSignature(DISPATCH_LINE_ROUTER);
  const b = normalizeSignature(DISPATCH_LINE_DEVTOOLS);
  assert.equal(a, b);
  assert.equal(a, 'emit package contract: unresolved obligation: ReactiveDispatchUnresolved { callee, member }');
});

test("normalizeSignature maps a type-facts failure to a different signature than the reactive-dispatch failure", () => {
  const dispatchSignature = normalizeSignature(DISPATCH_LINE_ROUTER);
  const typeFactsSignature = normalizeSignature(TYPE_FACTS_LINE);
  assert.notEqual(dispatchSignature, typeFactsSignature);
});

test("normalizeSignature erases versions", () => {
  const a = normalizeSignature("solid-js 1.9.14 failed");
  const b = normalizeSignature("solid-js 1.10.2-beta.3 failed");
  assert.equal(a, b);
  assert.equal(a, "solid-js failed");
});

test("normalizeSignature erases tmpdir names embedded in absolute paths", () => {
  const a = normalizeSignature("failure in /tmp/solid-checker-abc123/node_modules/pkg/file.js");
  const b = normalizeSignature("failure in /tmp/solid-checker-xyz789-other/node_modules/pkg/file.js");
  assert.equal(a, b);
});

// --- Defect fixes: scoped names must erase whole, the schema-v1 stub must
// never leak into a group key, and per-package identifiers must not leave
// two occurrences of the exact same real failure ungroupable. ---

test("DEFECT 1: a scoped package name is erased whole, not left with an orphaned scope", () => {
  const signature = normalizeSignature(CONDITIONAL_META_LINE);
  assert.ok(!signature.includes("@solidjs"), `signature must not contain the scope: ${signature}`);
  assert.ok(!signature.includes("solidjs"), `signature must not contain the package name: ${signature}`);
});

test("DEFECT 1: the conditional-export signature contains no package name, scope, export name, or branch literal", () => {
  const signature = normalizeSignature(CONDITIONAL_META_LINE);
  assert.ok(!signature.includes("@solidjs"), "no scope");
  assert.ok(!signature.includes("meta"), "no package name");
  assert.ok(!signature.includes("Stylesheet"), "no export name");
  assert.ok(!signature.includes('"solid"'), "no branch literal");
  assert.ok(!/\[\s*\]/.test(signature) || signature.includes("<conditions>"), "empty branch literal must be placeheld too");
});

test("DEFECT 2: two dependency-contract-obligation lines naming different scoped modules (@tanstack/query-core vs @corvu/accordion) produce the SAME signature", () => {
  const tanstack = classifyResult({ status: 2, stdout: "", stderr: EXPORT_ALL_TANSTACK, phase: "generate" });
  const corvu = classifyResult({ status: 2, stdout: "", stderr: EXPORT_ALL_CORVU, phase: "generate" });
  assert.equal(tanstack.class, "dependency-contract-obligation");
  assert.equal(corvu.class, "dependency-contract-obligation");
  assert.equal(tanstack.signature, corvu.signature);
  // detail.module (not the signature) is what still distinguishes the two
  // real blockers for the shared-dependency-blocker report.
  assert.equal(tanstack.detail.module, "@tanstack/query-core");
  assert.equal(corvu.detail.module, "@corvu/accordion");
});

test("DEFECT 3: the @solid-primitives/map unresolved-parameter-behavior signature is short and carries no JSON braces", () => {
  const result = classifyResult({ status: 2, stdout: "", stderr: PARAMETER_BEHAVIOR_MAP_LINE, phase: "generate" });
  assert.equal(result.class, "unresolved-parameter-behavior");
  assert.ok(result.signature.length < 120, `signature too long (${result.signature.length}): ${result.signature}`);
  assert.ok(!result.signature.includes("{") && !result.signature.includes("}"), `signature must contain no JSON braces: ${result.signature}`);
  // detail extraction (unchanged by this fix) still carries the exact values.
  assert.equal(result.detail.exportedFunction, "createMap");
  assert.equal(result.detail.parameter, 0);
  assert.equal(result.detail.parameterType, "any");
});

test("DEFECT 3: two unresolved-parameter-behavior failures from different packages, function names, and parameter types produce the SAME signature", () => {
  const a = classifyResult({ status: 2, stdout: "", stderr: PARAMETER_BEHAVIOR_MAP_LINE, phase: "generate" });
  const otherPackage =
    'solid-checker: solid-checker-rust: emit package contract: unresolved parameter behavior in createStore parameter 1 (string) at /tmp/y/node_modules/@solid-primitives/storage/dist/index.js:99: parameter 1 (string) is passed to resolved StorageAdapter.setItem from /tmp/y/node_modules/@solid-primitives/storage/dist/index.js, but no package contract proves when it executes; generate or add the exact package entrypoint/export contract before this callback can be certified; required behavior: choose exactly one audited mode: inline, tracked, or deferred; edit this schema-v1 stub and review its evidence: {"different":"stub","shape":true}';
  const b = classifyResult({ status: 2, stdout: "", stderr: otherPackage, phase: "generate" });
  assert.equal(a.class, "unresolved-parameter-behavior");
  assert.equal(b.class, "unresolved-parameter-behavior");
  assert.equal(a.signature, b.signature);
});

// The consumer-side contract obligations name the dependency whose contract is
// missing or environment-dependent. That name is what turns hundreds of
// separate failures into a handful of shared blockers in the report, so it is
// extracted into `detail.module` rather than left in the prose.
test("the consumer-side contract obligations expose their blocking module and export", () => {
  const environmentDependent = classifyResult({
    status: 2,
    stdout: "",
    stderr:
      'solid-checker: solid-checker-rust: emit package contract: unresolved obligation at /tmp/x/node_modules/@kobalte/core/dist/accordion/CxG9lD9Q.js:646: PackageContractEnvironmentDependent { module: "@solidjs/web", export: "createComponent", reexported: false }',
    phase: "generate"
  });
  assert.equal(environmentDependent.detail.module, "@solidjs/web");
  assert.equal(environmentDependent.detail.exportedFunction, "createComponent");
  assert.equal(environmentDependent.detail.dependency, "@kobalte/core");
  assert.equal(environmentDependent.detail.reexported, false);

  const exportMissing = classifyResult({
    status: 2,
    stdout: "",
    stderr:
      'solid-checker-rust: emit package contract: unresolved obligation at /tmp/x/node_modules/@tanstack/solid-router/dist/source/ClientOnly.jsx:1507: PackageContractExportMissing { module: "solid-js", export: "context", reexported: false }',
    phase: "generate"
  });
  assert.equal(exportMissing.detail.module, "solid-js");
  assert.equal(exportMissing.detail.exportedFunction, "context");
});

// Real stderr can carry an unrelated warning on its first line and the actual
// error on a later line prefixed only with `solid-checker-rust:`. If the outer
// `solid-checker:` prefix were required to strip it, the same failure would
// produce two different signatures depending on which line it landed on.
test("either checker prefix alone is stripped, so one failure yields one signature", () => {
  const body =
    'emit package contract: unresolved obligation at /tmp/a/node_modules/p/x.js:1: PackageContractExportMissing { module: "solid-js", export: "context", reexported: false }';
  const withBoth = classifyResult({ status: 2, stdout: "", stderr: `solid-checker: solid-checker-rust: ${body}`, phase: "generate" });
  const withInnerOnly = classifyResult({ status: 2, stdout: "", stderr: `solid-checker-rust: ${body}`, phase: "generate" });
  const withNeither = classifyResult({ status: 2, stdout: "", stderr: body, phase: "generate" });
  assert.equal(withBoth.signature, withInnerOnly.signature);
  assert.equal(withBoth.signature, withNeither.signature);
  assert.ok(!withBoth.signature.includes("solid-checker"), withBoth.signature);
});

// @solidjs/start's exports map advertises "./package.json"; the generator is
// handed a .json path and refuses it. That is a package-shape problem, not an
// unclassified mystery.
test("an unsupported source path from an exports map entry is a package-shape failure", () => {
  const result = classifyResult({
    status: 2,
    stdout: "",
    stderr:
      "solid-checker: solid-checker-rust: AST facts error: unsupported source path /tmp/x/node_modules/@solidjs/start/package.json: Unknown file extension: Please provide a valid file extension: .js, .mjs, .jsx or .cjs for JavaScript, or .ts, .d.ts, .mts, .cts or .tsx for TypeScript",
    phase: "generate"
  });
  assert.equal(result.class, "unsupported-package-shape");
});

// Captured from a real run after per-entrypoint partial contracts landed: the
// generator gained an environment-dependent-export-kind refusal, and an
// unclassified result is the signal that classification has to grow a case.
test("an environment-dependent export kind refusal is a conditional-summary incompatibility", () => {
  const result = classifyResult({
    status: 2,
    stdout: "",
    stderr:
      'solid-checker: @solid-devtools/locator has no certifiable runtime entrypoint; .: @solid-devtools/locator .:addClickInterceptor is kind "function" under conditions ["import"] but "value" elsewhere; schema v1 cannot represent an environment-dependent export kind, so split the entrypoint or review an explicit contract',
    phase: "generate"
  });
  assert.equal(result.class, "incompatible-conditional-summaries");
});

// The all-entrypoints-refused message embeds the underlying reason, so it must
// classify by that reason rather than falling through to `unclassified`.
test("an all-refused entrypoint message classifies by its embedded reason", () => {
  const result = classifyResult({
    status: 2,
    stdout: "",
    stderr:
      'solid-checker: pkg has no certifiable runtime entrypoint; ./x: solid-checker-rust: emit package contract: unresolved obligation at /tmp/a/node_modules/pkg/dist/i.js:1: ReactiveDispatchUnresolved { callee: "f", member: Some("map") }',
    phase: "generate"
  });
  assert.equal(result.class, "reactive-dispatch-unresolved");
});
