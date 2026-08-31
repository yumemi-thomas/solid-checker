// Standards-compatible standalone package acquisition for contract artifact
// resolution. This module produces exact records and never selects or rewrites
// contract semantics; Rust consumes the records at the normalization boundary.

import { createHash } from "node:crypto";
import { Buffer } from "node:buffer";
import {
  existsSync,
  lstatSync,
  readFileSync,
  readdirSync,
  realpathSync,
  statSync
} from "node:fs";
import { dirname, extname, join, relative, resolve, sep } from "node:path";
import tsNamespace from "typescript";

export function selectTypeScriptApi(namespace) {
  for (const candidate of [namespace, namespace?.default]) {
    if (
      typeof candidate?.createProgram === "function" &&
      candidate.ScriptTarget?.Latest !== undefined
    ) {
      return candidate;
    }
  }
  throw new TypeError("typescript module does not expose the compiler API");
}

const ts = selectTypeScriptApi(tsNamespace);

const RUNTIME_EXTENSIONS = [".js", ".mjs", ".cjs", ".jsx", ".ts", ".mts", ".cts", ".tsx"];
const DECLARATION_EXTENSIONS = [".d.ts", ".d.mts", ".d.cts"];
const DECLARATION_MODULE_EXTENSIONS = [
  ".ts",
  ".tsx",
  ".d.ts",
  ".mts",
  ".d.mts",
  ".cts",
  ".d.cts",
  ".js",
  ".jsx",
  ".mjs",
  ".cjs"
];
// The resolution-kind/type keys Node itself defines. These are active (or
// deliberately inactive) on every selection without a consumer naming them, so
// the condition census never enumerates one as an axis of its own.
export const RESOLVER_STANDARD_CONDITIONS = Object.freeze([
  "default",
  "types",
  "import",
  "require",
  "node-addons"
]);
export const MUTUALLY_EXCLUSIVE_CONDITION_AXES = Object.freeze([
  Object.freeze(["browser", "node", "deno", "worker"]),
  Object.freeze(["development", "production"]),
  Object.freeze(["csr", "string-ssr", "streaming-ssr"])
]);
// This checker's own ecosystem default. vite-plugin-solid and solid-start
// activate `solid` unconditionally, so a target behind it is reached by every
// real Solid consumer and a missing one is a defective publish rather than a
// private branch. It stays out of `RESOLVER_STANDARD_CONDITIONS` on purpose:
// the resolver does not activate it implicitly (Rust's replay does not either,
// and the two must select the same target), so the census still enumerates it
// as an axis and really exercises the branch.
const ECOSYSTEM_DEFAULT_CONDITIONS = Object.freeze(["solid"]);
// The exact condition vocabulary this resolver already understands without
// being told. The artifact-case disposition rules read it, so a condition is
// "custom" in exactly one place.
const STANDARD_CONDITION_NAMES = new Set([
  ...RESOLVER_STANDARD_CONDITIONS,
  ...MUTUALLY_EXCLUSIVE_CONDITION_AXES.flat(),
  ...ECOSYSTEM_DEFAULT_CONDITIONS
]);

/**
 * A package-export condition outside the standard set above. It says only
 * "this resolver does not itself define the name" — a bare custom name like
 * `bun`, `workerd`, or `react-native` is still activated unconditionally by the
 * ecosystem that owns it, so a consumer really does reach a target behind one.
 * Use `isPrivateNamespacedCondition` for the narrower "no consumer reaches this
 * without opting in" question.
 */
export function isCustomCondition(condition) {
  return !STANDARD_CONDITION_NAMES.has(condition);
}

/**
 * A custom condition whose name is namespaced — it contains a `/` or starts
 * with `@`, as in `@solid-devtools/source` or `vendor/source`. Namespacing is
 * the published convention for a condition private to one tool: a consumer
 * reaches a target behind it only by naming that exact condition in its own
 * resolver configuration. A bare-name custom condition is not private — an
 * ecosystem activates it for every one of its consumers — so it is deliberately
 * excluded here.
 */
export function isPrivateNamespacedCondition(condition) {
  return (
    isCustomCondition(condition) &&
    (condition.includes("/") || condition.startsWith("@"))
  );
}

// Extensions that name a resource nothing executes as a module: sourcemaps,
// data, stylesheets, images, fonts, and documents. This is deliberately a
// positive list rather than the complement of `RUNTIME_EXTENSIONS`: an
// entrypoint with an unknown, absent, or executable-but-opaque extension
// (`.node`, `.wasm`) is emphatically *not* "nothing to assert" — the closure
// already names those two as native-code/opaque-wasm hazards — so everything
// outside this list keeps ordinary certify-or-refuse semantics.
const NON_MODULE_EXTENSIONS = Object.freeze([
  ".map",
  ".json",
  ".css",
  ".svg",
  ".png",
  ".jpg",
  ".jpeg",
  ".gif",
  ".webp",
  ".ico",
  ".avif",
  ".woff",
  ".woff2",
  ".ttf",
  ".otf",
  ".eot",
  ".txt",
  ".md",
  ".html"
]);

/**
 * The extension of a target that is a genuinely non-executable resource, from
 * the exact `NON_MODULE_EXTENSIONS` list above. Everything else — a runtime
 * module extension, `.node`, `.wasm`, an unrecognized suffix, or no extension
 * at all — is answered `undefined` and stays on the ordinary path, where a
 * missing or unanalyzable entrypoint still refuses.
 */
export function nonModuleTargetExtension(path) {
  const extension = extname(path);
  if (!extension) return undefined;
  return NON_MODULE_EXTENSIONS.includes(extension.toLowerCase()) ? extension : undefined;
}

// These programs answer exactly one kind of question, in `syntaxHazards` and
// nowhere else: does the symbol this identifier resolves to have a declaration
// in *this same source file*? `noResolve` already keeps every imported module
// out, so the only non-root declarations a program could contribute are the
// default library's -- and a `lib.*.d.ts` declaration is never a declaration in
// an analyzed package file. Both answers a global identifier can produce, "the
// symbol is declared elsewhere" (libraries loaded) and "no symbol resolves at
// all" (libraries absent), therefore leave every predicate here reading `not
// locally bound`, while a genuinely local declaration resolves locally in both
// cases because lexical scope never consults globals.
//
// So `noLib` is a memory decision, not a semantic one, and it is a large one:
// the default `target: Latest` library set is 86 source files that were parsed
// and bound again for every module needing symbol identity, and it accounts for
// essentially the whole per-program cost -- ten retained programs measured
// 631 MB resident with the library set and ~0 without it, against roughly
// 70 MB per program observed in situ. It was checked as well as argued: across
// 6740 real installed package files (2507 of which need a checker) the hazard
// census is byte-identical with and without the library set.
const PROGRAM_OPTIONS = Object.freeze({
  allowJs: true,
  checkJs: true,
  noEmit: true,
  noLib: true,
  noResolve: true,
  skipLibCheck: true,
  target: ts.ScriptTarget.Latest
});
const DOMAIN_DEBUG = new Map([
  ["callbacks", "Callbacks"],
  ["reads", "Reads"],
  ["writes", "Writes"],
  ["creates", "Creates"],
  ["invalidates", "Invalidates"],
  ["throws", "Throws"],
  ["returns", "Returns"],
  ["cleanups", "Cleanups"],
  ["disposals", "Disposals"]
]);
const DOMAIN_NAMES = [...DOMAIN_DEBUG.keys()];
const ROLE_DEBUG = new Map([
  ["runtime", "Runtime"],
  ["declaration", "Declaration"],
  ["manifest", "Manifest"],
  ["resolution-input", "ResolutionInput"],
  ["literal-dynamic-chunk", "LiteralDynamicChunk"],
  ["generated", "Generated"]
]);
const ROLE_ORDER = new Map([...ROLE_DEBUG.keys()].map((value, index) => [value, index]));
const HAZARD_DEBUG = new Map([
  ["nonliteral-dynamic-loading", "NonliteralDynamicLoading"],
  ["eval", "Eval"],
  ["native-code", "NativeCode"],
  ["opaque-wasm", "OpaqueWasm"],
  ["mutable-unbound-global", "MutableUnboundGlobal"],
  ["unmaterialized-transform", "UnmaterializedTransform"],
  ["unaccepted-external-dependency", "UnacceptedExternalDependency"]
]);
const HAZARD_ORDER = new Map([...HAZARD_DEBUG.keys()].map((value, index) => [value, index]));

export class ArtifactResolutionError extends Error {
  constructor(code, message) {
    super(message);
    this.name = "ArtifactResolutionError";
    this.code = code;
  }
}

function fail(code, message) {
  throw new ArtifactResolutionError(code, message);
}

function sha256(bytes) {
  return `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
}

function fileDigest(path) {
  return sha256(readFileSync(path));
}

function isFile(path) {
  try {
    return statSync(path).isFile();
  } catch {
    return false;
  }
}

function realpath(path) {
  try {
    return realpathSync.native(path);
  } catch {
    return realpathSync(path);
  }
}

function slash(path) {
  return path.split(sep).join("/");
}

function packagePath(packageRoot, path) {
  const result = slash(relative(packageRoot, path));
  if (!result || result === ".." || result.startsWith("../")) {
    fail("invalid-target", `${path} is outside package root ${packageRoot}`);
  }
  return `./${result}`;
}

function pointerSegment(value) {
  return String(value).replaceAll("~", "~0").replaceAll("/", "~1");
}

function packageNameFromSpecifier(specifier) {
  if (!specifier || specifier.startsWith(".") || specifier.startsWith("/") || specifier.startsWith("#")) {
    fail("invalid-specifier", `${JSON.stringify(specifier)} is not a bare package specifier`);
  }
  const parts = specifier.split("/");
  return specifier.startsWith("@") ? parts.slice(0, 2).join("/") : parts[0];
}

function requestedEntrypoint(specifier, packageName) {
  const suffix = specifier.slice(packageName.length);
  return suffix ? `.${suffix}` : ".";
}

export function findPackageRoot(importer, packageName) {
  let directory = dirname(resolve(importer));
  while (true) {
    const candidate = join(directory, "node_modules", packageName);
    if (isFile(join(candidate, "package.json"))) return candidate;
    const parent = dirname(directory);
    if (parent === directory) break;
    directory = parent;
  }
  fail("package-not-found", `${packageName} is not installed above ${importer}`);
}

function targetKind(target) {
  if (target === null) return "null";
  if (typeof target === "string") return "string";
  if (Array.isArray(target)) return "array";
  if (target && typeof target === "object") return "conditions";
  return "invalid";
}

function validateTargetString(target, packageRoot) {
  if (!target.startsWith("./")) {
    fail("invalid-target", `package exports target ${JSON.stringify(target)} must start with "./"`);
  }
  const pieces = target.slice(2).split("/");
  if (
    pieces.some(
      piece =>
        piece === "" ||
        piece === "." ||
        piece === ".." ||
        piece === "node_modules" ||
        /%2e|%2f|%5c/i.test(piece)
    )
  ) {
    fail("invalid-target", `package exports target ${JSON.stringify(target)} contains an invalid segment`);
  }
  const path = resolve(packageRoot, target);
  packagePath(packageRoot, path);
  return path;
}

function patternCapture(pattern, candidate) {
  const star = pattern.indexOf("*");
  if (star < 0) return undefined;
  const prefix = pattern.slice(0, star);
  const suffix = pattern.slice(star + 1);
  if (!candidate.startsWith(prefix) || !candidate.endsWith(suffix)) return undefined;
  return candidate.slice(prefix.length, candidate.length - suffix.length);
}

function patternKeyCompare(left, right) {
  const leftStar = left.indexOf("*");
  const rightStar = right.indexOf("*");
  const leftBase = leftStar < 0 ? left.length : leftStar + 1;
  const rightBase = rightStar < 0 ? right.length : rightStar + 1;
  if (leftBase !== rightBase) return rightBase - leftBase;
  if (leftStar < 0 !== (rightStar < 0)) return leftStar < 0 ? -1 : 1;
  return right.length - left.length;
}

function selectSubpath(exportsField, entrypoint) {
  if (exportsField && typeof exportsField === "object" && !Array.isArray(exportsField)) {
    const keys = Object.keys(exportsField);
    const hasSubpath = keys.some(key => key.startsWith("."));
    const hasCondition = keys.some(key => !key.startsWith("."));
    if (hasSubpath && hasCondition) {
      fail("invalid-target", "package exports cannot mix subpath and condition keys");
    }
  }
  const explicit =
    exportsField &&
    typeof exportsField === "object" &&
    !Array.isArray(exportsField) &&
    Object.keys(exportsField).some(key => key.startsWith("."));
  const map = explicit ? exportsField : { ".": exportsField };
  if (Object.hasOwn(map, entrypoint)) {
    return { target: map[entrypoint], pointer: `/exports/${pointerSegment(entrypoint)}` };
  }
  const matches = Object.keys(map)
    .filter(key => key.startsWith(".") && key.includes("*") && patternCapture(key, entrypoint) !== undefined)
    .sort(patternKeyCompare);
  if (!matches.length) fail("not-exported", `${entrypoint} is not exported by the package`);
  const key = matches[0];
  return {
    target: map[key],
    pointer: `/exports/${pointerSegment(key)}`,
    capture: patternCapture(key, entrypoint)
  };
}

function selectPackageImport(importsField, specifier) {
  if (
    !importsField ||
    typeof importsField !== "object" ||
    Array.isArray(importsField)
  ) {
    fail("package-import-not-defined", `${specifier} is not defined by the package imports map`);
  }
  if (specifier === "#" || specifier.startsWith("#/")) {
    fail("invalid-specifier", `${JSON.stringify(specifier)} is not a valid package imports specifier`);
  }
  if (Object.hasOwn(importsField, specifier)) {
    return {
      target: importsField[specifier],
      pointer: `/imports/${pointerSegment(specifier)}`
    };
  }
  const matches = Object.keys(importsField)
    .filter(
      key =>
        key.startsWith("#") &&
        key.includes("*") &&
        patternCapture(key, specifier) !== undefined
    )
    .sort(patternKeyCompare);
  if (!matches.length) {
    fail("package-import-not-defined", `${specifier} is not defined by the package imports map`);
  }
  const key = matches[0];
  return {
    target: importsField[key],
    pointer: `/imports/${pointerSegment(key)}`,
    capture: patternCapture(key, specifier)
  };
}

function substitutePattern(target, capture) {
  return capture === undefined ? target : target.replaceAll("*", capture);
}

function selectTarget(target, context) {
  switch (targetKind(target)) {
    case "null":
      fail("blocked", `${context.entrypoint} is blocked by a null package target`);
      break;
    case "string": {
      const selected = substitutePattern(target, context.capture);
      const path = validateTargetString(selected, context.packageRoot);
      return {
        path,
        branch: context.pointer,
        steps: [...context.steps, { condition: "target", target: selected }],
        // Exact condition keys traversed to reach this target. Deliberately not
        // part of `steps`: the trace is hashed into the resolution record, and
        // reading condition names back out of it would confuse a package
        // condition literally named `subpath`, `array`, or `target` with the
        // resolver's own step markers.
        conditions: context.conditionsTaken
      };
    }
    case "array": {
      let lastInvalid;
      for (let index = 0; index < target.length; index += 1) {
        try {
          return selectTarget(target[index], {
            ...context,
            pointer: `${context.pointer}/${index}`,
            steps: [...context.steps, { condition: "array", target: String(index) }]
          });
        } catch (error) {
          if (!(error instanceof ArtifactResolutionError) || error.code !== "invalid-target") throw error;
          lastInvalid = error;
        }
      }
      throw lastInvalid ?? new ArtifactResolutionError("invalid-target", "package target array is empty");
    }
    case "conditions": {
      const keys = Object.keys(target);
      const hasSubpath = keys.some(key => key.startsWith("."));
      if (hasSubpath) fail("invalid-target", "subpath keys cannot be nested inside a conditional target");
      // Node's PACKAGE_TARGET_RESOLVE continues to the next key when a key's
      // own target resolves to nothing, so `{"vendor": {"browser": "./a.js"},
      // "default": "./index.js"}` under conditions ["vendor"] resolves to
      // ./index.js. Taking the first *matching* key and refusing there instead
      // would report a defect where every real consumer resolves fine. Only a
      // nested `conditions-unmatched` backtracks: `null` (blocked) and an
      // invalid target are properties of the package and still refuse
      // immediately, exactly as Node's own algorithm treats them.
      //
      // The abandoned branch's `steps` and `conditionsTaken` are discarded,
      // never merged: `context` is copied per key rather than mutated, so the
      // trace that survives is exactly the branch actually taken. Recording a
      // condition the selection walked away from would put a name in the
      // resolution record's hashed trace that no consumer's resolution ever
      // traverses.
      for (const condition of keys) {
        if (condition !== "default" && !context.conditions.has(condition)) continue;
        try {
          return selectTarget(target[condition], {
            ...context,
            pointer: `${context.pointer}/${pointerSegment(condition)}`,
            steps: [...context.steps, { condition, target: context.pointer }],
            conditionsTaken: [...context.conditionsTaken, condition]
          });
        } catch (error) {
          if (
            !(error instanceof ArtifactResolutionError) ||
            error.code !== "conditions-unmatched"
          ) {
            throw error;
          }
        }
      }
      fail("conditions-unmatched", `${context.entrypoint} selects no active package-export condition`);
      break;
    }
    default:
      fail("invalid-target", "package target must be a string, object, array, or null");
  }
}

function declarationCandidate(path) {
  if (DECLARATION_EXTENSIONS.some(extension => path.endsWith(extension))) return isFile(path) ? path : undefined;
  const extension = extname(path);
  const stem = extension ? path.slice(0, -extension.length) : path;
  const candidates = [
    ...DECLARATION_EXTENSIONS.map(candidate => `${stem}${candidate}`),
    ...DECLARATION_EXTENSIONS.map(candidate => join(path, `index${candidate}`))
  ];
  return candidates.find(isFile) ?? (RUNTIME_EXTENSIONS.some(candidate => path.endsWith(candidate)) && isFile(path) ? path : undefined);
}

function resolvedFile(path) {
  if (!isFile(path)) fail("target-not-found", `resolved target ${path} is not a file`);
  const real = realpath(path);
  return {
    path,
    ...(real !== path ? { realPath: real } : {}),
    digest: fileDigest(real)
  };
}

// The runtime target a legacy `module` field names, when it names a real file.
// `module` is the bundler's ESM entry of a dual package whose `main` is usually
// the CJS transpile of the same source, so the runtime axis prefers it instead
// of refusing the package. A declared-but-absent (or unresolvable) target is not
// a refusal: Node consumers never read `module`, so the `main` surface is still
// real. The segment check mirrors the Rust snapshot replay's `module`
// preference exactly, so the generator and the certifier select the same field.
function legacyModuleTarget(packageRoot, manifest) {
  if (typeof manifest.module !== "string") return undefined;
  const pieces = manifest.module.replace(/^(?:\.\/)+/, "").split("/");
  if (
    pieces.some(
      piece =>
        piece === "" ||
        piece === "." ||
        piece === ".." ||
        piece === "node_modules" ||
        piece.includes("\\") ||
        /%2e|%2f|%5c/i.test(piece)
    )
  ) {
    return undefined;
  }
  const path = resolve(packageRoot, manifest.module);
  return isFile(path) ? path : undefined;
}

/**
 * Exact export-map target selection, stopping before the selected path is
 * required to be a real file. `resolvePackageExport` is this plus that
 * requirement; the artifact-case disposition rules need the selection alone,
 * because "the target this branch names is absent" and "the target this branch
 * names is not a module" are both properties of the selection.
 */
export function selectPackageExportTarget({
  packageRoot,
  manifest,
  entrypoint = ".",
  conditions = [],
  axis = "runtime",
  resolutionKind = "import"
}) {
  const active = new Set([...conditions, resolutionKind, "default"]);
  if (axis === "declarations") active.add("types");
  else active.delete("types");

  if (manifest.exports !== undefined) {
    const selected = selectSubpath(manifest.exports, entrypoint);
    const target = selectTarget(selected.target, {
      packageRoot,
      entrypoint,
      capture: selected.capture,
      conditions: active,
      pointer: selected.pointer,
      steps: [{ condition: "subpath", target: entrypoint }],
      conditionsTaken: []
    });
    const path = axis === "declarations" ? declarationCandidate(target.path) : target.path;
    if (!path) fail("declarations-not-found", `no declaration target exists for ${target.path}`);
    return {
      path,
      exists: isFile(path),
      trace: { branch: target.branch, steps: target.steps },
      conditions: target.conditions,
      legacyField: null
    };
  }

  if (entrypoint !== ".") fail("not-exported", `${entrypoint} has no legacy package entrypoint`);
  const fallback = "index.js";
  const field =
    axis === "declarations"
      ? manifest.types
        ? "types"
        : manifest.typings
          ? "typings"
          : manifest.main
            ? "main"
            : "index"
      : legacyModuleTarget(packageRoot, manifest)
        ? "module"
        : manifest.main
          ? "main"
          : "index";
  const target = field === "index" ? fallback : manifest[field];
  const initial = resolve(packageRoot, target);
  const path = axis === "declarations" ? declarationCandidate(initial) : initial;
  if (!path) fail("declarations-not-found", `no declaration target exists for ${initial}`);
  return {
    path,
    exists: isFile(path),
    trace: {
      branch: `legacy:${field}`,
      steps: [{ condition: field, target }]
    },
    // A legacy field name is not a package-export condition; no consumer opts
    // into `main` or `module`, so this selection never took a custom condition.
    conditions: [],
    legacyField: field
  };
}

export function resolvePackageExport(request) {
  const selected = selectPackageExportTarget(request);
  return { file: resolvedFile(selected.path), trace: selected.trace };
}

function resolvePackageImport({
  packageRoot,
  manifest,
  specifier,
  conditions = [],
  axis = "runtime",
  resolutionKind = "import"
}) {
  const active = new Set([...conditions, resolutionKind, "default"]);
  if (axis === "declarations") active.add("types");
  else active.delete("types");
  const selected = selectPackageImport(manifest.imports, specifier);
  const target = selectTarget(selected.target, {
    packageRoot,
    entrypoint: specifier,
    capture: selected.capture,
    conditions: active,
    pointer: selected.pointer,
    steps: [{ condition: "imports", target: specifier }],
    conditionsTaken: []
  });
  const path = axis === "declarations" ? declarationCandidate(target.path) : target.path;
  if (!path) fail("declarations-not-found", `no declaration target exists for ${target.path}`);
  return path;
}

// A `#` specifier whose conditional target matches none of this partition's
// active conditions is unknown here, not absent. The condition census names an
// environment axis only when a partition selects one, so the partition that
// names none cannot say which of `browser`/`node`/... a consumer will activate,
// while every real environment activates one of them. Answering that with a
// refusal of the whole artifact case would treat "this census row selects no
// arm" as proof that no runtime module executes. Leave the specifier
// unresolved instead: its module stays outside the closure and every claim
// reachable from the importing module stays open. Every other imports-map
// failure (undefined specifier, `null` block, missing target, invalid target)
// is a property of the package rather than of the census row, and still
// refuses.
function packageImportTargetOrUnknown(request) {
  try {
    return resolvePackageImport(request);
  } catch (error) {
    if (error instanceof ArtifactResolutionError && error.code === "conditions-unmatched") {
      return undefined;
    }
    throw error;
  }
}

function parseModule(path) {
  const program = ts.createProgram({
    rootNames: [path],
    options: PROGRAM_OPTIONS
  });
  const file = program.getSourceFile(path);
  if (!file) fail("module-parse", `TypeScript did not include resolved module ${path}`);
  return {
    file,
    checker: program.getTypeChecker()
  };
}

function sourceNeedsChecker(sourceFile) {
  let required = false;
  const visit = node => {
    if (required) return;
    if (
      ts.isCallExpression(node) &&
      ts.isIdentifier(node.expression) &&
      (node.expression.text === "require" || node.expression.text === "eval")
    ) {
      required = true;
      return;
    }
    if (
      ts.isPropertyAccessExpression(node) &&
      ts.isIdentifier(node.expression) &&
      node.expression.text === "WebAssembly"
    ) {
      required = true;
      return;
    }
    if (
      ts.isBinaryExpression(node) &&
      node.operatorToken.kind >= ts.SyntaxKind.FirstAssignment &&
      node.operatorToken.kind <= ts.SyntaxKind.LastAssignment
    ) {
      required = true;
      return;
    }
    if (
      (ts.isForInStatement(node) || ts.isForOfStatement(node)) &&
      !ts.isVariableDeclarationList(node.initializer)
    ) {
      required = true;
      return;
    }
    if (
      (ts.isPrefixUnaryExpression(node) || ts.isPostfixUnaryExpression(node)) &&
      (node.operator === ts.SyntaxKind.PlusPlusToken || node.operator === ts.SyntaxKind.MinusMinusToken) &&
      ts.isIdentifier(node.operand)
    ) {
      required = true;
      return;
    }
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);
  return required;
}

function isProgramSource(path) {
  return [...DECLARATION_EXTENSIONS, ...RUNTIME_EXTENSIONS].some(extension =>
    path.endsWith(extension)
  );
}

function packageProgramSources(packageRoot) {
  const paths = [];
  const visit = directory => {
    const entries = readdirSync(directory, { withFileTypes: true }).sort((left, right) =>
      left.name.localeCompare(right.name)
    );
    for (const entry of entries) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) {
        if (entry.name !== "node_modules") visit(path);
      } else if ((entry.isFile() || entry.isSymbolicLink()) && isProgramSource(path) && isFile(path)) {
        paths.push(path);
      }
    }
  };
  visit(packageRoot);
  return paths;
}

class PackageModuleParser {
  #packageRoot;
  #records = new Map();
  #program;
  #checker;
  #programsCreated = 0;

  constructor(packageRoot) {
    this.#packageRoot = packageRoot;
    this.#rebuild();
  }

  #rebuild() {
    this.#records = new Map();
    for (const path of packageProgramSources(this.#packageRoot)) {
      const real = realpath(path);
      const bytes = readFileSync(real);
      const file = ts.createSourceFile(path, bytes.toString("utf8"), ts.ScriptTarget.Latest, true);
      this.#records.set(path, {
        digest: sha256(bytes),
        file,
        needsChecker: sourceNeedsChecker(file),
        externalModule: ts.isExternalModule(file)
      });
    }
    const roots = [...this.#records]
      .filter(([, record]) => record.needsChecker && record.externalModule)
      .map(([path]) => path);
    if (roots.length === 0) {
      this.#program = undefined;
      this.#checker = undefined;
      return;
    }
    const host = ts.createCompilerHost(PROGRAM_OPTIONS);
    const defaultGetSourceFile = host.getSourceFile.bind(host);
    host.getSourceFile = (fileName, ...arguments_) =>
      this.#records.get(fileName)?.file ?? defaultGetSourceFile(fileName, ...arguments_);
    this.#program = ts.createProgram({ rootNames: roots, options: PROGRAM_OPTIONS, host });
    this.#checker = this.#program.getTypeChecker();
    this.#programsCreated += 1;
  }

  parse(path, digest) {
    let record = this.#records.get(path);
    if (record?.digest !== digest) {
      this.#rebuild();
      record = this.#records.get(path);
    }
    if (!record) {
      this.#programsCreated += 1;
      return parseModule(path);
    }
    if (!record.needsChecker) return { file: record.file, checker: null };
    if (record.externalModule) {
      const file = this.#program?.getSourceFile(path);
      if (!file || !this.#checker) fail("module-parse", `TypeScript did not include resolved module ${path}`);
      return { file, checker: this.#checker };
    }
    this.#programsCreated += 1;
    return parseModule(path);
  }

  programsCreated() {
    return this.#programsCreated;
  }
}

// Recursive dependency planning usually touches only a package's selected
// entrypoint closure. Parsing every source in that package (the certification
// transaction's correct strategy) is needless work here. This parser remains
// semantically exact for visited files: files needing symbol identity use the
// ordinary isolated TypeScript program, while syntax-only files avoid a
// checker altogether.
class LazyClosureModuleParser {
  parse(path, digest) {
    const bytes = readFileSync(realpath(path));
    if (sha256(bytes) !== digest) {
      fail("module-parse", `resolved module ${path} changed during dependency planning`);
    }
    const file = ts.createSourceFile(path, bytes.toString("utf8"), ts.ScriptTarget.Latest, true);
    return sourceNeedsChecker(file) ? parseModule(path) : { file, checker: null };
  }
}

function hasModifier(node, kind) {
  return node.modifiers?.some(modifier => modifier.kind === kind) ?? false;
}

function isLocallyBoundIdentifier(node, checker, sourceFile) {
  const symbol = checker.getSymbolAtLocation(node);
  return symbol?.declarations?.some(declaration => declaration.getSourceFile() === sourceFile) ?? false;
}

function mutationTargetHasUnboundIdentifier(node, checker, sourceFile) {
  while (
    ts.isParenthesizedExpression(node) ||
    ts.isAsExpression(node) ||
    ts.isTypeAssertionExpression(node) ||
    ts.isNonNullExpression(node) ||
    ts.isSatisfiesExpression(node)
  ) {
    node = node.expression;
  }
  if (ts.isIdentifier(node)) return !isLocallyBoundIdentifier(node, checker, sourceFile);
  if (ts.isArrayLiteralExpression(node)) {
    return node.elements.some(element =>
      ts.isOmittedExpression(element)
        ? false
        : mutationTargetHasUnboundIdentifier(
            ts.isSpreadElement(element) ? element.expression : element,
            checker,
            sourceFile
          )
    );
  }
  if (ts.isObjectLiteralExpression(node)) {
    return node.properties.some(property => {
      if (ts.isShorthandPropertyAssignment(property)) {
        const symbol = checker.getShorthandAssignmentValueSymbol(property);
        return !symbol?.declarations?.some(
          declaration => declaration.getSourceFile() === sourceFile
        );
      }
      if (ts.isPropertyAssignment(property)) {
        return mutationTargetHasUnboundIdentifier(property.initializer, checker, sourceFile);
      }
      if (ts.isSpreadAssignment(property)) {
        return mutationTargetHasUnboundIdentifier(property.expression, checker, sourceFile);
      }
      return false;
    });
  }
  if (
    ts.isBinaryExpression(node) &&
    node.operatorToken.kind >= ts.SyntaxKind.FirstAssignment &&
    node.operatorToken.kind <= ts.SyntaxKind.LastAssignment
  ) {
    return mutationTargetHasUnboundIdentifier(node.left, checker, sourceFile);
  }
  return false;
}

// A Vite-style resource query (`./x.js?raw`, `?url`, `?worker`) or URL fragment
// makes an import bundler-mediated. The binding's value is whatever the loader
// produces -- a string for `?raw`, a URL string for `?url`, a constructor for
// `?worker` -- and never the target module's exports, so stripping the suffix
// and walking into the module would assert the wrong semantics for the binding.
// Refusing instead would treat a file the package ships as missing and kill the
// whole artifact case. The specifier is therefore opaque on both sides: no
// closure edge, no resolved binding, and the same unaccepted-external frontier
// a bare unaccepted dependency records, so every claim reachable through the
// binding stays open.
//
// Edges, all deliberate: a `#` at index 0 is a package imports specifier rather
// than a fragment, so only a `#` later in the specifier is a suffix; a suffix
// introducer with nothing after it (`./x.js?`) names nothing a loader treats
// specially and stays on the ordinary path, where a missing file still refuses;
// a bare specifier carrying a suffix (`pkg/x.css?inline`) is opaque too and is
// deliberately kept out of the external dependency census, because no package
// entrypoint answers to a suffixed subpath; and a literal on-disk filename
// containing `?` or `#` stays unreachable from a specifier, because no real
// loader resolves one.
function bundlerResourceSuffix(specifier) {
  const query = specifier.indexOf("?");
  const fragment = specifier.indexOf("#", 1);
  const introducer = query < 0 ? fragment : fragment < 0 ? query : Math.min(query, fragment);
  return introducer > 0 && introducer < specifier.length - 1
    ? specifier.slice(introducer)
    : undefined;
}

function localModuleTarget(importer, specifier, axis, packageRoot) {
  if (bundlerResourceSuffix(specifier)) return undefined;
  if (!specifier.startsWith(".") && !specifier.startsWith("/")) return undefined;
  const base = specifier.startsWith("/") ? specifier : resolve(dirname(importer), specifier);
  const observedExtension = extname(base);
  const allowedExtensions = axis === "runtime"
    ? RUNTIME_EXTENSIONS
    : [...DECLARATION_EXTENSIONS, ...RUNTIME_EXTENSIONS];
  // A real file with an unrecognized suffix is an asset. A dotted basename
  // that does not exist yet (for example HeadContent.dev) is still eligible
  // for normal module suffix resolution.
  if (observedExtension && !allowedExtensions.includes(observedExtension) && isFile(base)) {
    return undefined;
  }
  const extension = allowedExtensions.includes(observedExtension) ? observedExtension : "";
  const sourceSubstitutions =
    extension === ".js"
      ? [`${base.slice(0, -3)}.ts`, `${base.slice(0, -3)}.tsx`, `${base.slice(0, -3)}.d.ts`]
      : extension === ".jsx"
        ? [`${base.slice(0, -4)}.tsx`, `${base.slice(0, -4)}.d.ts`]
        : extension === ".mjs"
          ? [`${base.slice(0, -4)}.mts`, `${base.slice(0, -4)}.d.mts`]
          : extension === ".cjs"
            ? [`${base.slice(0, -4)}.cts`, `${base.slice(0, -4)}.d.cts`]
            : [];
  const declarationSourceSubstitutions =
    extension === ".ts" || extension === ".tsx"
      ? [`${base.slice(0, -extension.length)}.d.ts`]
      : extension === ".mts"
        ? [`${base.slice(0, -4)}.d.mts`]
        : extension === ".cts"
          ? [`${base.slice(0, -4)}.d.cts`]
          : [];
  const candidates = axis === "runtime"
    ? extension
      ? [base, ...sourceSubstitutions]
      : [base, ...RUNTIME_EXTENSIONS.map(value => `${base}${value}`), ...RUNTIME_EXTENSIONS.map(value => join(base, `index${value}`))]
    : extension
      ? DECLARATION_EXTENSIONS.includes(extension) || [".ts", ".tsx", ".mts", ".cts"].includes(extension)
        ? [base, ...declarationSourceSubstitutions]
        : [...sourceSubstitutions, declarationCandidate(base)].filter(Boolean)
      : [
          base,
          ...DECLARATION_MODULE_EXTENSIONS.map(value => `${base}${value}`),
          ...DECLARATION_MODULE_EXTENSIONS.map(value => join(base, `index${value}`))
        ];
  const selected = candidates.find(isFile);
  if (!selected) return undefined;
  packagePath(packageRoot, selected);
  return selected;
}

function localAssetTarget(importer, specifier, packageRoot) {
  if (bundlerResourceSuffix(specifier)) return undefined;
  if (!specifier.startsWith(".") && !specifier.startsWith("/")) return undefined;
  const path = specifier.startsWith("/") ? specifier : resolve(dirname(importer), specifier);
  if (!isFile(path)) return undefined;
  packagePath(packageRoot, path);
  const extension = extname(path);
  return RUNTIME_EXTENSIONS.includes(extension) || DECLARATION_EXTENSIONS.includes(extension)
    ? undefined
    : path;
}

function packageScopeForImporter(importer, packageRoot, cache) {
  const root = resolve(packageRoot);
  const start = dirname(resolve(importer));
  const relativeStart = relative(root, start);
  if (relativeStart === ".." || relativeStart.startsWith(`..${sep}`)) {
    fail("invalid-target", `${importer} is outside package root ${packageRoot}`);
  }
  const scopes = cache.packageScopes ??= new Map();
  const cached = scopes.get(start);
  if (cached) return cached;

  const visited = [];
  let directory = start;
  while (true) {
    const known = scopes.get(directory);
    if (known) {
      for (const path of visited) scopes.set(path, known);
      return known;
    }
    visited.push(directory);
    const manifestPath = join(directory, "package.json");
    if (isFile(manifestPath)) {
      const bytes = readFileSync(manifestPath);
      const scope = {
        packageRoot: directory,
        manifest:
          directory === root
            ? cache.resolutionProgram.manifest
            : JSON.parse(bytes),
        digest: sha256(bytes)
      };
      for (const path of visited) scopes.set(path, scope);
      return scope;
    }
    if (directory === root) break;
    const parent = dirname(directory);
    if (parent === directory) break;
    directory = parent;
  }
  fail("package-identity", `no package manifest owns importer ${importer}`);
}

function moduleTarget(importer, specifier, axis, packageRoot, cache) {
  if (bundlerResourceSuffix(specifier)) return undefined;
  if (specifier.startsWith("#")) {
    const scope = packageScopeForImporter(importer, packageRoot, cache);
    return packageImportTargetOrUnknown({
      packageRoot: scope.packageRoot,
      manifest: scope.manifest,
      specifier,
      conditions: cache.resolutionProgram.conditions,
      axis,
      resolutionKind: cache.resolutionProgram.resolutionKind
    });
  }
  return localModuleTarget(importer, specifier, axis, packageRoot);
}

function moduleDescription(path, axis, packageRoot, cache) {
  const scope = packageScopeForImporter(path, packageRoot, cache);
  const key = `${cache.resolutionProgram.key}:${scope.digest}:${axis}:${path}`;
  if (cache.local.has(key)) return cache.local.get(key);
  const digest = fileDigest(realpath(path));
  const shared = cache.shared?.get(key);
  if (shared?.digest === digest) {
    cache.local.set(key, shared.description);
    return shared.description;
  }
  const { file, checker } = cache.parser?.parse(path, digest) ?? parseModule(path);
  const description = {
    direct: new Map(),
    stars: [],
    externalDirect: new Map(),
    externalStars: [],
    imports: new Map(),
    externalImports: new Map(),
    specifiers: [],
    // The syntax hazard census is extracted here, while the TypeScript program
    // that answers it is still live, and only its plain-data result is retained
    // (see the `hazards` note below).
    hazards: []
  };
  cache.local.set(key, description);
  cache.shared?.set(key, { digest, description });
  for (const statement of file.statements) {
    if (ts.isImportDeclaration(statement) && ts.isStringLiteral(statement.moduleSpecifier)) {
      const target = moduleTarget(path, statement.moduleSpecifier.text, axis, packageRoot, cache);
      description.specifiers.push({
        kind: "import",
        text: statement.moduleSpecifier.text,
        target,
        asset: target ? undefined : localAssetTarget(path, statement.moduleSpecifier.text, packageRoot)
      });
      if (!statement.importClause || statement.importClause.isTypeOnly) continue;
      if (statement.importClause.name) {
        if (target) {
          description.imports.set(statement.importClause.name.text, {
            file: target,
            name: "default"
          });
        } else {
          description.externalImports.set(statement.importClause.name.text, {
            specifier: statement.moduleSpecifier.text,
            name: "default"
          });
        }
      }
      const bindings = statement.importClause.namedBindings;
      if (bindings && ts.isNamedImports(bindings)) {
        for (const element of bindings.elements) {
          if (element.isTypeOnly) continue;
          const imported = element.propertyName?.text ?? element.name.text;
          if (target) {
            description.imports.set(element.name.text, { file: target, name: imported });
          } else {
            description.externalImports.set(element.name.text, {
              specifier: statement.moduleSpecifier.text,
              name: imported
            });
          }
        }
      }
    }
    if (ts.isExportDeclaration(statement)) {
      const module = statement.moduleSpecifier;
      const target = module && ts.isStringLiteral(module)
        ? moduleTarget(path, module.text, axis, packageRoot, cache)
        : undefined;
      if (module && ts.isStringLiteral(module)) {
        description.specifiers.push({
          kind: "reexport",
          text: module.text,
          target,
          asset: target ? undefined : localAssetTarget(path, module.text, packageRoot)
        });
      }
      if (statement.isTypeOnly) continue;
      if (!statement.exportClause) {
        if (target) description.stars.push(target);
        else if (module && ts.isStringLiteral(module)) {
          description.externalStars.push(module.text);
        }
        continue;
      }
      if (ts.isNamespaceExport(statement.exportClause)) {
        if (target) description.direct.set(statement.exportClause.name.text, { file: target, name: "*" });
        else if (module && ts.isStringLiteral(module)) {
          description.externalDirect.set(statement.exportClause.name.text, {
            specifier: module.text,
            name: "*"
          });
        }
        continue;
      }
      for (const element of statement.exportClause.elements) {
        if (element.isTypeOnly) continue;
        const publicName = element.name.text;
        const localName = element.propertyName?.text ?? publicName;
        if (target) description.direct.set(publicName, { file: target, name: localName });
        else if (module && ts.isStringLiteral(module)) {
          description.externalDirect.set(publicName, {
            specifier: module.text,
            name: localName
          });
        } else {
          const externalImport = description.externalImports.get(localName);
          if (externalImport) description.externalDirect.set(publicName, externalImport);
          else {
            description.direct.set(
              publicName,
              description.imports.get(localName) ?? { file: path, name: localName }
            );
          }
        }
      }
      continue;
    }
    if (ts.isExportAssignment(statement)) {
      description.direct.set("default", { file: path, name: "default" });
      continue;
    }
    if (!hasModifier(statement, ts.SyntaxKind.ExportKeyword)) continue;
    if (hasModifier(statement, ts.SyntaxKind.DefaultKeyword)) {
      description.direct.set("default", { file: path, name: "default" });
    }
    if (
      (ts.isFunctionDeclaration(statement) ||
        ts.isClassDeclaration(statement) ||
        ts.isEnumDeclaration(statement)) &&
      statement.name
    ) {
      description.direct.set(statement.name.text, { file: path, name: statement.name.text });
    }
    if (ts.isVariableStatement(statement)) {
      for (const declaration of statement.declarationList.declarations) {
        if (ts.isIdentifier(declaration.name)) {
          description.direct.set(declaration.name.text, { file: path, name: declaration.name.text });
        }
      }
    }
  }
  const visitDynamic = node => {
    if (
      ts.isCallExpression(node) &&
      (node.expression.kind === ts.SyntaxKind.ImportKeyword ||
        (ts.isIdentifier(node.expression) &&
          node.expression.text === "require" &&
          !isLocallyBoundIdentifier(node.expression, checker, file))) &&
      node.arguments.length === 1 &&
      ts.isStringLiteralLike(node.arguments[0])
    ) {
      const text = node.arguments[0].text;
      const opaque = text.endsWith(".wasm") || text.endsWith(".node");
      description.specifiers.push({
        kind: "dynamic",
        text,
        target: opaque ? undefined : moduleTarget(path, text, axis, packageRoot, cache),
        asset: opaque ? undefined : localAssetTarget(path, text, packageRoot),
        dynamic: node.expression.kind === ts.SyntaxKind.ImportKeyword
      });
    }
    ts.forEachChild(node, visitDynamic);
  };
  visitDynamic(file);
  const externallyForwarded = new Set(
    [...description.externalDirect.values()].map(binding => binding.specifier)
  );
  for (const specifier of description.specifiers) {
    if (specifier.kind === "import" && externallyForwarded.has(specifier.text)) {
      specifier.kind = "reexport";
    }
  }
  // A cached description must not keep the TypeScript program that produced it
  // alive. `parseModule` builds a whole standalone `ts.Program` per module that
  // needs symbol identity, and every module description used to retain that
  // program transitively through its `checker` (and its `SourceFile`). One
  // session's description cache therefore held one live program per such
  // module -- measured at roughly 70 MB each, and dozens of them across a
  // dependency graph's preparation.
  //
  // The only consumer of the AST and checker beyond this function is the
  // syntax hazard census, which is a pure function of (relative path, file
  // text) and is recomputed identically on every closure walk. Extract it once
  // here, while the program is still live, and retain only its plain-data
  // result. `closureForRoots` copies the rows it consumes, so the census stays
  // exactly as immutable to its callers as a freshly computed one.
  description.hazards = syntaxHazards(packagePath(packageRoot, path), file, checker);
  return description;
}

function acceptedExternalBinding(acceptedDependencies, specifier, name, axis) {
  if (name === "*") return undefined;
  const binding = acceptedDependencies[specifier]?.exports?.[name]?.[axis];
  if (!binding) return undefined;
  if (
    typeof binding.exportName !== "string" ||
    !binding.exportName ||
    typeof binding.module?.path !== "string" ||
    !binding.module.path ||
    typeof binding.module?.digest !== "string" ||
    !/^sha256:[0-9a-f]{64}$/.test(binding.module.digest)
  ) {
    fail(
      "accepted-dependency-binding",
      `accepted dependency ${specifier} has an invalid ${axis} binding for export ${name}`
    );
  }
  return binding;
}

function bindExport(
  path,
  name,
  axis,
  packageRoot,
  cache,
  acceptedDependencies,
  visiting = new Set()
) {
  const identity = `${axis}:${path}:${name}`;
  if (visiting.has(identity)) fail("export-cycle", `export ${name} participates in a re-export cycle`);
  visiting.add(identity);
  const description = moduleDescription(path, axis, packageRoot, cache);
  const direct = description.direct.get(name);
  if (direct) {
    if (direct.file === path || direct.name === "*") {
      visiting.delete(identity);
      return { module: resolvedFile(direct.file), exportName: direct.name };
    }
    const result = bindExport(
      direct.file,
      direct.name,
      axis,
      packageRoot,
      cache,
      acceptedDependencies,
      visiting
    );
    visiting.delete(identity);
    return result;
  }
  const externalDirect = description.externalDirect.get(name);
  if (externalDirect) {
    const result = acceptedExternalBinding(
      acceptedDependencies,
      externalDirect.specifier,
      externalDirect.name,
      axis
    );
    if (!result) {
      fail(
        "accepted-dependency-binding",
        `accepted dependency ${externalDirect.specifier} has no exact ${axis} binding for export ${externalDirect.name}`
      );
    }
    visiting.delete(identity);
    return result;
  }
  if (name === "default") {
    visiting.delete(identity);
    return undefined;
  }
  const candidates = [
    ...description.stars.map(target =>
      bindExport(target, name, axis, packageRoot, cache, acceptedDependencies, visiting)
    ),
    ...description.externalStars.map(specifier =>
      acceptedExternalBinding(acceptedDependencies, specifier, name, axis)
    )
  ]
    .filter(Boolean);
  visiting.delete(identity);
  const unique = new Map(candidates.map(candidate => [`${candidate.module.digest}:${candidate.exportName}`, candidate]));
  if (unique.size > 1) fail("ambiguous-export", `export ${name} resolves through multiple star exports`);
  return unique.values().next().value;
}

function exportedNames(
  path,
  axis,
  packageRoot,
  cache,
  acceptedDependencies,
  visiting = new Set()
) {
  const identity = `${axis}:${path}`;
  if (visiting.has(identity)) return new Set();
  visiting.add(identity);
  const description = moduleDescription(path, axis, packageRoot, cache);
  const names = new Set(description.direct.keys());
  for (const name of description.externalDirect.keys()) names.add(name);
  for (const target of description.stars) {
    for (const name of exportedNames(
      target,
      axis,
      packageRoot,
      cache,
      acceptedDependencies,
      visiting
    )) {
      if (name !== "default") names.add(name);
    }
  }
  for (const specifier of description.externalStars) {
    for (const name of Object.keys(acceptedDependencies[specifier]?.exports ?? {})) {
      if (name !== "default") names.add(name);
    }
  }
  visiting.delete(identity);
  return names;
}

function exactExportBindings(
  runtime,
  declarations,
  packageRoot,
  sharedCache,
  parser,
  acceptedDependencies,
  resolutionProgram
) {
  const cache = { local: new Map(), shared: sharedCache, parser, resolutionProgram };
  const runtimeNames = exportedNames(
    runtime.path,
    "runtime",
    packageRoot,
    cache,
    acceptedDependencies
  );
  const declarationNames = exportedNames(
    declarations.path,
    "declarations",
    packageRoot,
    cache,
    acceptedDependencies
  );
  const names = [...runtimeNames].filter(name => declarationNames.has(name)).sort();
  const exports = {};
  for (const name of names) {
    const runtimeTarget = bindExport(
      runtime.path,
      name,
      "runtime",
      packageRoot,
      cache,
      acceptedDependencies
    );
    const declarationTarget = bindExport(
      declarations.path,
      name,
      "declarations",
      packageRoot,
      cache,
      acceptedDependencies
    );
    if (!runtimeTarget || !declarationTarget) continue;
    exports[name] = { runtime: runtimeTarget, declarations: declarationTarget };
  }
  return { exports, cache };
}

function syntaxHazards(path, sourceFile, checker) {
  const hazards = [];
  const byteOffset = offset => Buffer.byteLength(sourceFile.text.slice(0, offset), "utf8");
  const add = (kind, node) =>
    hazards.push({
      kind,
      source: `${path}:${byteOffset(node.getStart(sourceFile))}-${byteOffset(node.end)}`,
      affectedExports: [],
      affectedDomains: [...DOMAIN_NAMES]
    });
  const visit = node => {
    if (ts.isCallExpression(node)) {
      if (node.expression.kind === ts.SyntaxKind.ImportKeyword) {
        if (node.arguments.length !== 1 || !ts.isStringLiteralLike(node.arguments[0])) {
          add("nonliteral-dynamic-loading", node);
        }
      } else if (
        ts.isIdentifier(node.expression) &&
        node.expression.text === "require" &&
        !isLocallyBoundIdentifier(node.expression, checker, sourceFile)
      ) {
        if (node.arguments.length !== 1 || !ts.isStringLiteralLike(node.arguments[0])) {
          add("nonliteral-dynamic-loading", node);
        }
      } else if (
        ts.isIdentifier(node.expression) &&
        node.expression.text === "eval" &&
        !isLocallyBoundIdentifier(node.expression, checker, sourceFile)
      ) {
        add("eval", node);
      }
    }
    if (
      ts.isPropertyAccessExpression(node) &&
      ts.isIdentifier(node.expression) &&
      node.expression.text === "WebAssembly" &&
      !isLocallyBoundIdentifier(node.expression, checker, sourceFile)
    ) {
      add("opaque-wasm", node);
    }
    if (
      ts.isBinaryExpression(node) &&
      node.operatorToken.kind >= ts.SyntaxKind.FirstAssignment &&
      node.operatorToken.kind <= ts.SyntaxKind.LastAssignment &&
      mutationTargetHasUnboundIdentifier(node.left, checker, sourceFile)
    ) {
      add("mutable-unbound-global", node);
    }
    if (
      (ts.isForInStatement(node) || ts.isForOfStatement(node)) &&
      !ts.isVariableDeclarationList(node.initializer) &&
      mutationTargetHasUnboundIdentifier(node.initializer, checker, sourceFile)
    ) {
      add("mutable-unbound-global", node.initializer);
    }
    if (
      (ts.isPrefixUnaryExpression(node) || ts.isPostfixUnaryExpression(node)) &&
      (node.operator === ts.SyntaxKind.PlusPlusToken || node.operator === ts.SyntaxKind.MinusMinusToken) &&
      ts.isIdentifier(node.operand) &&
      !isLocallyBoundIdentifier(node.operand, checker, sourceFile)
    ) {
      add("mutable-unbound-global", node);
    }
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);
  return hazards;
}

function closureForRoots(
  packageRoot,
  manifestPath,
  runtime,
  declarations,
  cache,
  acceptedDependencies = {},
  externalDependencyCensus = null
) {
  const entries = new Map();
  const dependencies = new Map();
  const hazards = [];
  const manifestIdentity = {
    path: packagePath(packageRoot, manifestPath),
    digest: fileDigest(realpath(manifestPath))
  };
  entries.set(`manifest:${manifestIdentity.path}`, { role: "manifest", ...manifestIdentity });
  entries.set(`resolution-input:${manifestIdentity.path}`, {
    role: "resolution-input",
    ...manifestIdentity
  });
  const visit = (path, axis, roleOverride) => {
    const role = roleOverride ?? (axis === "runtime" ? "runtime" : "declaration");
    const relativePath = packagePath(packageRoot, path);
    const key = `${role}:${relativePath}`;
    if (entries.has(key)) return;
    entries.set(key, { role, path: relativePath, digest: fileDigest(realpath(path)) });
    const description = moduleDescription(path, axis, packageRoot, cache);
    // Copy each cached census row: `canonicalClosure` sorts and deduplicates
    // the arrays it is handed in place, and a cached row is shared with every
    // other closure walk over the same module.
    hazards.push(
      ...description.hazards.map(hazard => ({
        ...hazard,
        affectedExports: [...hazard.affectedExports],
        affectedDomains: [...hazard.affectedDomains]
      }))
    );
    for (const specifier of description.specifiers) {
      if (specifier.asset) {
        const assetPath = packagePath(packageRoot, specifier.asset);
        entries.set(`resolution-input:${assetPath}`, {
          role: "resolution-input",
          path: assetPath,
          digest: fileDigest(realpath(specifier.asset))
        });
        continue;
      }
      if (specifier.target) {
        // A matched `#` specifier reaching a local module is exactly where the
        // generator and the certifier disagree: this closure walks into the
        // imports-map target, while the Rust replay has no imports-map support
        // at all (`SnapshotPackageManifest` carries no `imports` field and
        // `resolve_local` treats every `#specifier` as External), so it would
        // reject the resulting proposal with a closure mismatch. Refuse the
        // artifact case here instead of emitting a proposal that cannot
        // certify. Teaching Rust the imports map is the named follow-up in
        // docs/precision-backlog.md; until then this is fail-closed.
        if (specifier.text.startsWith("#")) {
          fail(
            "package-imports-unsupported",
            `package imports-map target ${packagePath(packageRoot, specifier.target)} ` +
              `resolves into the closure; certifier replay does not support imports maps yet`
          );
        }
        visit(
          specifier.target,
          axis,
          role === "literal-dynamic-chunk"
            ? role
            : specifier.dynamic && axis === "runtime"
              ? "literal-dynamic-chunk"
              : undefined
        );
        continue;
      }
      // Bundler-mediated asset import: opaque, never a closure edge, never a
      // census row. See `bundlerResourceSuffix`.
      if (bundlerResourceSuffix(specifier.text)) {
        hazards.push({
          kind: "unaccepted-external-dependency",
          source: `${relativePath}:${specifier.text}`,
          affectedExports: [],
          affectedDomains: [...DOMAIN_NAMES]
        });
        continue;
      }
      if (specifier.text.endsWith(".node") || specifier.text.endsWith(".wasm")) {
        hazards.push({
          kind: specifier.text.endsWith(".node") ? "native-code" : "opaque-wasm",
          source: `${relativePath}:${specifier.text}`,
          affectedExports: [],
          affectedDomains: [...DOMAIN_NAMES]
        });
        continue;
      }
      if (specifier.text.startsWith(".") || specifier.text.startsWith("/")) {
        fail("module-not-found", `local closure module ${specifier.text} from ${path} was not found`);
      }
      // An unmatched `#` specifier is unknown in this census row, not absent:
      // see `packageImportTargetOrUnknown`. It is emphatically not an external
      // dependency — `#platform` names no package, and putting it in the census
      // made the external locator derive a nonsense package name from it. Give
      // it the same unresolved/opaque frontier the other two closure builders
      // give it, so every claim reachable through the binding stays open.
      if (specifier.text.startsWith("#")) {
        hazards.push({
          kind: "unaccepted-external-dependency",
          source: `${relativePath}:${specifier.text}`,
          affectedExports: [],
          affectedDomains: [...DOMAIN_NAMES]
        });
        continue;
      }
      externalDependencyCensus?.push({
        axis,
        importerPath: relativePath,
        kind: specifier.kind,
        specifier: specifier.text
      });
      const accepted = acceptedDependencies[specifier.text];
      if (accepted) {
        dependencies.set(`${specifier.text}:${accepted.packageName}`, {
          specifier: specifier.text,
          packageName: accepted.packageName,
          artifactCase: accepted.artifactCase,
          acceptedContractDigest: accepted.acceptedContractDigest
        });
      } else {
        hazards.push({
          kind: specifier.text.endsWith(".node")
            ? "native-code"
            : specifier.text.endsWith(".wasm")
              ? "opaque-wasm"
              : "unaccepted-external-dependency",
          source: `${relativePath}:${specifier.text}`,
          affectedExports: [],
          affectedDomains: [...DOMAIN_NAMES]
        });
      }
    }
  };
  visit(runtime.path, "runtime");
  visit(declarations.path, "declarations");
  return canonicalClosure([...entries.values()], [...dependencies.values()], hazards);
}

function compareText(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

function compareTextArrays(left, right) {
  for (let index = 0; index < Math.min(left.length, right.length); index += 1) {
    const order = compareText(left[index], right[index]);
    if (order) return order;
  }
  return left.length - right.length;
}

function canonicalSha256(value, field) {
  if (typeof value !== "string" || !/^sha256:[0-9a-f]{64}$/i.test(value)) {
    fail("invalid-digest", `${field} must be sha256 followed by 64 hexadecimal digits`);
  }
  return value.toLowerCase();
}

export function canonicalClosure(entries = [], dependencies = [], hazards = []) {
  for (const entry of entries) {
    if (!ROLE_DEBUG.has(entry.role)) fail("invalid-closure", `unknown closure role ${entry.role}`);
    entry.digest = canonicalSha256(entry.digest, `closure entry ${entry.path}`);
    if (entry.role === "generated") {
      entry.transformDigest = canonicalSha256(
        entry.transformDigest,
        `generated closure entry ${entry.path} transform`
      );
    } else if (entry.transformDigest !== undefined) {
      fail("invalid-closure", `non-generated closure entry ${entry.path} has a transform digest`);
    }
  }
  for (const dependency of dependencies) {
    dependency.acceptedContractDigest = canonicalSha256(
      dependency.acceptedContractDigest,
      `dependency ${dependency.specifier}`
    );
  }
  entries.sort(
    (left, right) =>
      ROLE_ORDER.get(left.role) - ROLE_ORDER.get(right.role) ||
      compareText(left.path, right.path) ||
      compareText(left.digest, right.digest) ||
      compareText(left.transformDigest ?? "", right.transformDigest ?? "")
  );
  dependencies.sort(
    (left, right) =>
      compareText(left.specifier, right.specifier) ||
      compareText(left.packageName, right.packageName) ||
      compareText(left.artifactCase, right.artifactCase) ||
      compareText(left.acceptedContractDigest, right.acceptedContractDigest)
  );
  for (const hazard of hazards) {
    if (!HAZARD_DEBUG.has(hazard.kind)) {
      fail("invalid-closure", `unknown closure hazard ${hazard.kind}`);
    }
    if (!hazard.affectedDomains.length) {
      fail("invalid-closure", `closure hazard ${hazard.kind} names no affected domain`);
    }
    if (hazard.affectedDomains.some(domain => !DOMAIN_DEBUG.has(domain))) {
      fail("invalid-closure", `closure hazard ${hazard.kind} names an unknown affected domain`);
    }
    hazard.affectedExports = [...new Set(hazard.affectedExports)].sort();
    hazard.affectedDomains = [...new Set(hazard.affectedDomains)].sort(
      (left, right) => DOMAIN_NAMES.indexOf(left) - DOMAIN_NAMES.indexOf(right)
    );
  }
  hazards = [
    ...new Map(
      hazards.map(hazard => [
        JSON.stringify([
          hazard.kind,
          hazard.source,
          hazard.affectedExports,
          hazard.affectedDomains
        ]),
        hazard
      ])
    ).values()
  ];
  hazards.sort(
    (left, right) =>
      HAZARD_ORDER.get(left.kind) - HAZARD_ORDER.get(right.kind) ||
      compareText(left.source, right.source) ||
      compareTextArrays(left.affectedExports, right.affectedExports) ||
      compareTextArrays(left.affectedDomains, right.affectedDomains)
  );
  const hash = createHash("sha256");
  const count = value => {
    const bytes = Buffer.alloc(8);
    bytes.writeBigUInt64BE(BigInt(value));
    hash.update(bytes);
  };
  const text = value => {
    const bytes = Buffer.from(value);
    count(bytes.length);
    hash.update(bytes);
  };
  const optional = value => {
    hash.update(Buffer.from([value === undefined ? 0 : 1]));
    if (value !== undefined) text(value);
  };
  hash.update("solid-checker:artifact-closure:v1");
  count(entries.length);
  for (const entry of entries) {
    text(ROLE_DEBUG.get(entry.role));
    text(entry.path);
    text(entry.digest);
    optional(entry.transformDigest);
  }
  count(dependencies.length);
  for (const dependency of dependencies) {
    text(dependency.specifier);
    text(dependency.packageName);
    text(dependency.artifactCase);
    text(dependency.acceptedContractDigest);
  }
  count(hazards.length);
  for (const hazard of hazards) {
    text(HAZARD_DEBUG.get(hazard.kind));
    text(hazard.source);
    count(hazard.affectedExports.length);
    for (const name of hazard.affectedExports) text(name);
    count(hazard.affectedDomains.length);
    for (const domain of hazard.affectedDomains) text(DOMAIN_DEBUG.get(domain));
  }
  return { entries, dependencies, hazards, digest: `sha256:${hash.digest("hex")}` };
}

export function resolvePackageArtifacts({
  importer,
  specifier,
  packageRoot,
  conditions = [],
  resolutionKind = "import",
  integrity,
  acceptedDependencies = {}
}, session = null) {
  const packageName = packageNameFromSpecifier(specifier);
  const logicalRoot = resolve(packageRoot ?? findPackageRoot(importer, packageName));
  const manifestPath = join(logicalRoot, "package.json");
  const manifestBytes = readFileSync(manifestPath);
  const manifest = JSON.parse(manifestBytes);
  if (manifest.name !== packageName) {
    fail("package-identity", `resolved manifest declares ${JSON.stringify(manifest.name)}, expected ${packageName}`);
  }
  if (!manifest.version) fail("package-identity", "resolved manifest declares no version");
  if (!integrity) fail("package-identity", "standalone resolution requires exact package integrity");
  const entrypoint = requestedEntrypoint(specifier, packageName);
  const runtime = resolvePackageExport({
    packageRoot: logicalRoot,
    manifest,
    entrypoint,
    conditions,
    axis: "runtime",
    resolutionKind
  });
  const declarations = resolvePackageExport({
    packageRoot: logicalRoot,
    manifest,
    entrypoint,
    conditions,
    axis: "declarations",
    resolutionKind
  });
  const resolutionProgram = {
    manifest,
    conditions: [...new Set(conditions)].sort(),
    resolutionKind,
    key: JSON.stringify([
      sha256(manifestBytes),
      manifest.imports === undefined ? [] : [...new Set(conditions)].sort(),
      manifest.imports === undefined ? null : resolutionKind
    ])
  };
  const semanticKey = JSON.stringify([
    logicalRoot,
    sha256(manifestBytes),
    runtime.file.path,
    runtime.file.digest,
    declarations.file.path,
    declarations.file.digest,
    resolutionProgram.key,
    Object.entries(acceptedDependencies)
      .sort(([left], [right]) => compareText(left, right))
      .map(([dependency, accepted]) => [
        dependency,
        accepted.packageName,
        accepted.artifactCase,
        accepted.acceptedContractDigest,
        accepted.exports
      ])
  ]);
  let semantic = session?.[SESSION_LOOKUP](semanticKey, logicalRoot);
  if (!semantic) {
    const { exports, cache } = exactExportBindings(
      runtime.file,
      declarations.file,
      logicalRoot,
      session?.[SESSION_MODULE_CACHE](),
      session?.[SESSION_MODULE_PARSER](logicalRoot),
      acceptedDependencies,
      resolutionProgram
    );
    const closure = closureForRoots(
      logicalRoot,
      manifestPath,
      runtime.file,
      declarations.file,
      cache,
      acceptedDependencies
    );
    semantic = { exports, closure };
    session?.[SESSION_STORE](semanticKey, semantic);
  }
  const realRoot = realpath(logicalRoot);
  return {
    specifier,
    importer: resolve(importer),
    requestedEntrypoint: entrypoint,
    packageName,
    packageVersion: manifest.version,
    packageIntegrity: integrity,
    packageRoot: logicalRoot,
    ...(realRoot !== logicalRoot ? { packageRealRoot: realRoot } : {}),
    packageManifest: {
      path: manifestPath,
      ...(realpath(manifestPath) !== manifestPath ? { realPath: realpath(manifestPath) } : {}),
      digest: sha256(manifestBytes)
    },
    runtime: runtime.file,
    declarations: declarations.file,
    runtimeTrace: runtime.trace,
    declarationTrace: declarations.trace,
    closure: semantic.closure,
    exports: semantic.exports,
    authority: "standalonePackageResolver"
  };
}

// Dependency certification planning needs the exact runtime/declaration
// closure even when an external export-all prevents export binding. Keep that
// operation separate from `resolvePackageArtifacts`: it deliberately returns
// no exports and therefore cannot be mistaken for a certifiable artifact.
export function resolvePackageArtifactClosure({
  importer,
  specifier,
  packageRoot,
  conditions = [],
  resolutionKind = "import",
  integrity
}, session = null) {
  const packageName = packageNameFromSpecifier(specifier);
  const logicalRoot = resolve(packageRoot ?? findPackageRoot(importer, packageName));
  const manifestPath = join(logicalRoot, "package.json");
  const manifestBytes = readFileSync(manifestPath);
  const manifest = JSON.parse(manifestBytes);
  if (manifest.name !== packageName) {
    fail("package-identity", `resolved manifest declares ${JSON.stringify(manifest.name)}, expected ${packageName}`);
  }
  if (!manifest.version) fail("package-identity", "resolved manifest declares no version");
  if (!integrity) fail("package-identity", "dependency planning requires exact package integrity");
  const entrypoint = requestedEntrypoint(specifier, packageName);
  const runtime = resolvePackageExport({
    packageRoot: logicalRoot,
    manifest,
    entrypoint,
    conditions,
    axis: "runtime",
    resolutionKind
  });
  const declarations = resolvePackageExport({
    packageRoot: logicalRoot,
    manifest,
    entrypoint,
    conditions,
    axis: "declarations",
    resolutionKind
  });
  const resolutionProgram = {
    manifest,
    conditions: [...new Set(conditions)].sort(),
    resolutionKind,
    key: JSON.stringify([
      sha256(manifestBytes),
      manifest.imports === undefined ? [] : [...new Set(conditions)].sort(),
      manifest.imports === undefined ? null : resolutionKind
    ])
  };
  const cache = {
    local: new Map(),
    shared: session?.[SESSION_MODULE_CACHE](),
    parser: new LazyClosureModuleParser(),
    resolutionProgram
  };
  const externalDependencies = [];
  const closure = closureForRoots(
    logicalRoot,
    manifestPath,
    runtime.file,
    declarations.file,
    cache,
    {},
    externalDependencies
  );
  const canonicalExternalDependencies = [...new Map(
    externalDependencies.map(dependency => [
      `${dependency.axis}\0${dependency.importerPath}\0${dependency.kind}\0${dependency.specifier}`,
      dependency
    ])
  ).values()].sort((left, right) =>
    compareText(left.axis, right.axis) ||
    compareText(left.importerPath, right.importerPath) ||
    compareText(left.kind, right.kind) ||
    compareText(left.specifier, right.specifier)
  );
  return {
    specifier,
    importer: resolve(importer),
    requestedEntrypoint: entrypoint,
    packageName,
    packageVersion: manifest.version,
    packageIntegrity: integrity,
    packageRoot: logicalRoot,
    runtime: runtime.file,
    declarations: declarations.file,
    closure,
    externalDependencies: canonicalExternalDependencies
  };
}

function dependencyPlanningClosure(
  packageRoot,
  manifestPath,
  manifest,
  runtime,
  declarations,
  conditions,
  resolutionKind
) {
  const entries = new Map();
  const external = new Map();
  const frontiers = new Map();
  const addEntry = (role, path) => {
    const relativePath = packagePath(packageRoot, path);
    const entry = { role, path: relativePath, digest: fileDigest(realpath(path)) };
    entries.set(`${role}:${relativePath}`, entry);
    return entry;
  };
  addEntry("manifest", manifestPath);
  const visit = (path, axis, role = axis === "runtime" ? "runtime" : "declaration") => {
    const relativePath = packagePath(packageRoot, path);
    const key = `${role}:${relativePath}`;
    if (entries.has(key)) return;
    addEntry(role, path);
    const source = readFileSync(realpath(path), "utf8");
    const file = ts.createSourceFile(path, source, ts.ScriptTarget.Latest, true);
    const byteOffset = offset => Buffer.byteLength(source.slice(0, offset), "utf8");
    const follow = (specifier, dynamic = false) => {
      // Bundler-mediated asset import: opaque, never a closure edge. Record the
      // same unaccepted external frontier the fall-through below records for a
      // bare specifier. See `bundlerResourceSuffix`.
      if (bundlerResourceSuffix(specifier)) {
        external.set(`${relativePath}:${specifier}`, {
          kind: "unaccepted-external-dependency",
          source: `${relativePath}:${specifier}`,
          affectedExports: [],
          affectedDomains: [...DOMAIN_NAMES]
        });
        return;
      }
      if (specifier.startsWith("#")) {
        const selected = packageImportTargetOrUnknown({
          packageRoot,
          manifest,
          specifier,
          conditions,
          axis,
          resolutionKind
        });
        if (selected) {
          visit(
            selected,
            axis,
            role === "literal-dynamic-chunk" || (dynamic && axis === "runtime")
              ? "literal-dynamic-chunk"
              : undefined
          );
          return;
        }
        // Unknown in this census row: record the same unaccepted external
        // frontier the fall-through below records for a bare specifier, so
        // every domain of every reachable export stays open.
        external.set(`${relativePath}:${specifier}`, {
          kind: "unaccepted-external-dependency",
          source: `${relativePath}:${specifier}`,
          affectedExports: [],
          affectedDomains: [...DOMAIN_NAMES]
        });
        return;
      }
      const target = localModuleTarget(path, specifier, axis, packageRoot);
      if (target) {
        visit(
          target,
          axis,
          role === "literal-dynamic-chunk" || (dynamic && axis === "runtime")
            ? "literal-dynamic-chunk"
            : undefined
        );
        return;
      }
      const asset = localAssetTarget(path, specifier, packageRoot);
      if (asset) {
        addEntry("resolution-input", asset);
        return;
      }
      if (specifier.startsWith(".") || specifier.startsWith("/")) {
        fail("module-not-found", `local closure module ${specifier} from ${path} was not found`);
      }
      external.set(`${relativePath}:${specifier}`, {
        kind: "unaccepted-external-dependency",
        source: `${relativePath}:${specifier}`,
        affectedExports: [],
        affectedDomains: [...DOMAIN_NAMES]
      });
    };
    for (const statement of file.statements) {
      if (
        (ts.isImportDeclaration(statement) || ts.isExportDeclaration(statement)) &&
        statement.moduleSpecifier &&
        ts.isStringLiteralLike(statement.moduleSpecifier)
      ) {
        follow(statement.moduleSpecifier.text);
      } else if (
        ts.isImportEqualsDeclaration(statement) &&
        ts.isExternalModuleReference(statement.moduleReference) &&
        statement.moduleReference.expression &&
        ts.isStringLiteralLike(statement.moduleReference.expression)
      ) {
        follow(statement.moduleReference.expression.text);
      }
    }
    const visitDynamic = node => {
      if (ts.isCallExpression(node) && node.expression.kind === ts.SyntaxKind.ImportKeyword) {
        if (node.arguments.length === 1 && ts.isStringLiteralLike(node.arguments[0])) {
          follow(node.arguments[0].text, true);
        } else {
          const sourceId = `${relativePath}:${byteOffset(node.getStart(file))}-${byteOffset(node.end)}`;
          frontiers.set(sourceId, { kind: "nonliteral-dynamic-loading", source: sourceId });
        }
      } else if (
        ts.isCallExpression(node) &&
        ts.isIdentifier(node.expression) &&
        node.expression.text === "require"
      ) {
        // Knowing whether `require` is the ambient loader or a lexical binding
        // needs checker authority. Stop at that exact source location instead
        // of rebuilding a package-wide Program or guessing an external edge.
        const specifier =
          node.arguments.length === 1 && ts.isStringLiteralLike(node.arguments[0])
            ? node.arguments[0].text
            : null;
        const sourceId = `${relativePath}:${byteOffset(node.getStart(file))}-${byteOffset(node.end)}`;
        frontiers.set(sourceId, {
          kind: "semantic-require-binding",
          source: sourceId,
          specifier
        });
      }
      ts.forEachChild(node, visitDynamic);
    };
    visitDynamic(file);
  };
  visit(runtime.path, "runtime");
  visit(declarations.path, "declarations");
  const sortedEntries = [...entries.values()].sort((left, right) =>
    left.role.localeCompare(right.role) ||
    left.path.localeCompare(right.path) ||
    left.digest.localeCompare(right.digest)
  );
  const hazards = [...external.values()].sort((left, right) => left.source.localeCompare(right.source));
  const sortedFrontiers = [...frontiers.values()].sort((left, right) => left.source.localeCompare(right.source));
  const digest = `sha256:${createHash("sha256")
    .update("solid-checker:dependency-planning-closure:v1\0")
    .update(JSON.stringify({ entries: sortedEntries, hazards, frontiers: sortedFrontiers }))
    .digest("hex")}`;
  return { entries: sortedEntries, hazards, frontiers: sortedFrontiers, digest };
}

// A fast, fail-closed graph-planning acquisition. Static ESM edges are exact;
// CommonJS `require` stops at a source-bound frontier because resolving its
// lexical identity would require the full certification checker program.
export function resolvePackageDependencyPlanClosure({
  importer,
  specifier,
  packageRoot,
  conditions = [],
  resolutionKind = "import",
  integrity
}) {
  const packageName = packageNameFromSpecifier(specifier);
  const logicalRoot = resolve(packageRoot ?? findPackageRoot(importer, packageName));
  const manifestPath = join(logicalRoot, "package.json");
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  if (manifest.name !== packageName) {
    fail("package-identity", `resolved manifest declares ${JSON.stringify(manifest.name)}, expected ${packageName}`);
  }
  if (!manifest.version) fail("package-identity", "resolved manifest declares no version");
  if (!integrity) fail("package-identity", "dependency planning requires exact package integrity");
  const entrypoint = requestedEntrypoint(specifier, packageName);
  const runtime = resolvePackageExport({
    packageRoot: logicalRoot,
    manifest,
    entrypoint,
    conditions,
    axis: "runtime",
    resolutionKind
  });
  const declarations = resolvePackageExport({
    packageRoot: logicalRoot,
    manifest,
    entrypoint,
    conditions,
    axis: "declarations",
    resolutionKind
  });
  return {
    specifier,
    requestedEntrypoint: entrypoint,
    packageName,
    packageVersion: manifest.version,
    packageIntegrity: integrity,
    packageRoot: logicalRoot,
    runtime: runtime.file,
    declarations: declarations.file,
    closure: dependencyPlanningClosure(
      logicalRoot,
      manifestPath,
      manifest,
      runtime.file,
      declarations.file,
      conditions,
      resolutionKind
    )
  };
}

// One proposal-generation transaction may resolve several condition cases for
// the same immutable exact package artifact. The session is the narrow owner
// of any transaction-local acquisition reuse; callers still receive complete
// standalone resolution records and cannot supply cached semantic material.
const SESSION_LOOKUP = Symbol("artifact-resolution-session-lookup");
const SESSION_STORE = Symbol("artifact-resolution-session-store");
const SESSION_MODULE_CACHE = Symbol("artifact-resolution-session-module-cache");
const SESSION_MODULE_PARSER = Symbol("artifact-resolution-session-module-parser");

class SharedModuleDescriptionCache extends Map {
  descriptionsParsed = 0;

  set(key, value) {
    this.descriptionsParsed += 1;
    return super.set(key, value);
  }
}

export function closureEntriesAreCurrent(entries, packageRoot) {
  if (!Array.isArray(entries) || entries.length === 0) return false;
  const expected = new Map();
  for (const entry of entries) {
    if (
      typeof entry?.path !== "string" ||
      !entry.path.startsWith("./") ||
      typeof entry.digest !== "string"
    ) {
      return false;
    }
    const previous = expected.get(entry.path);
    if (previous && previous !== entry.digest) return false;
    expected.set(entry.path, entry.digest);
  }
  try {
    for (const [path, digest] of expected) {
      if (fileDigest(realpath(resolve(packageRoot, path))) !== digest) return false;
    }
  } catch {
    return false;
  }
  return true;
}

function semanticClosureIsCurrent(semantic, packageRoot) {
  return closureEntriesAreCurrent(semantic.closure.entries, packageRoot);
}

export class ArtifactResolutionSession {
  #requests = 0;
  #semanticAcquisitions = 0;
  #semanticCacheHits = 0;
  #semanticCacheInvalidations = 0;
  #semanticCache = new Map();
  #moduleDescriptions = new SharedModuleDescriptionCache();
  #moduleParsers = new Map();

  resolve(options) {
    this.#requests += 1;
    return resolvePackageArtifacts(options, this);
  }

  [SESSION_LOOKUP](key, packageRoot) {
    const cached = this.#semanticCache.get(key);
    if (!cached) return undefined;
    if (!semanticClosureIsCurrent(cached, packageRoot)) {
      this.#semanticCache.delete(key);
      this.#semanticCacheInvalidations += 1;
      return undefined;
    }
    this.#semanticCacheHits += 1;
    return structuredClone(cached);
  }

  [SESSION_STORE](key, semantic) {
    this.#semanticAcquisitions += 1;
    this.#semanticCache.set(key, structuredClone(semantic));
  }

  [SESSION_MODULE_CACHE]() {
    return this.#moduleDescriptions;
  }

  [SESSION_MODULE_PARSER](packageRoot) {
    let parser = this.#moduleParsers.get(packageRoot);
    if (!parser) {
      parser = new PackageModuleParser(packageRoot);
      this.#moduleParsers.set(packageRoot, parser);
    }
    return parser;
  }

  statistics() {
    return {
      requests: this.#requests,
      semanticAcquisitions: this.#semanticAcquisitions,
      semanticCacheHits: this.#semanticCacheHits,
      semanticCacheInvalidations: this.#semanticCacheInvalidations,
      moduleDescriptionsParsed: this.#moduleDescriptions.descriptionsParsed,
      typeScriptProgramsCreated: [...this.#moduleParsers.values()].reduce(
        (count, parser) => count + parser.programsCreated(),
        0
      )
    };
  }
}

export function materializedGeneratedClosureEntry({ stableId, bytes, transformDigest }) {
  if (!stableId || !transformDigest?.startsWith("sha256:")) {
    fail("unmaterialized-transform", "generated output requires stable bytes and transform digest");
  }
  return {
    role: "generated",
    path: `virtual:${stableId}`,
    digest: sha256(bytes),
    transformDigest
  };
}

export function isSymlinkedPackageRoot(path) {
  return lstatSync(path).isSymbolicLink() || realpath(path) !== resolve(path);
}
