// Standards-compatible standalone package acquisition for contract artifact
// resolution. This module produces exact records and never selects or rewrites
// contract semantics; Rust consumes the records at the normalization boundary.

import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  readFileSync,
  realpathSync,
  statSync
} from "node:fs";
import { createRequire } from "node:module";
import { dirname, extname, join, relative, resolve, sep } from "node:path";

const require = createRequire(import.meta.url);
const ts = require("typescript");

const RUNTIME_EXTENSIONS = [".js", ".mjs", ".cjs", ".jsx", ".ts", ".mts", ".cts", ".tsx"];
const DECLARATION_EXTENSIONS = [".d.ts", ".d.mts", ".d.cts"];
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
        steps: [...context.steps, { condition: "target", target: selected }]
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
      for (const condition of keys) {
        if (condition !== "default" && !context.conditions.has(condition)) continue;
        return selectTarget(target[condition], {
          ...context,
          pointer: `${context.pointer}/${pointerSegment(condition)}`,
          steps: [...context.steps, { condition, target: context.pointer }]
        });
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

export function resolvePackageExport({
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
      steps: [{ condition: "subpath", target: entrypoint }]
    });
    const path = axis === "declarations" ? declarationCandidate(target.path) : target.path;
    if (!path) fail("declarations-not-found", `no declaration target exists for ${target.path}`);
    return { file: resolvedFile(path), trace: { branch: target.branch, steps: target.steps } };
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
      : manifest.main
        ? "main"
        : "index";
  const target = field === "index" ? fallback : manifest[field];
  const initial = resolve(packageRoot, target);
  const path = axis === "declarations" ? declarationCandidate(initial) : initial;
  if (!path) fail("declarations-not-found", `no declaration target exists for ${initial}`);
  return {
    file: resolvedFile(path),
    trace: {
      branch: `legacy:${field}`,
      steps: [{ condition: field, target }]
    }
  };
}

function parseModule(path) {
  const program = ts.createProgram({
    rootNames: [path],
    options: {
      allowJs: true,
      checkJs: true,
      noEmit: true,
      noResolve: true,
      skipLibCheck: true,
      target: ts.ScriptTarget.Latest
    }
  });
  const file = program.getSourceFile(path);
  if (!file) fail("module-parse", `TypeScript did not include resolved module ${path}`);
  return {
    file,
    checker: program.getTypeChecker()
  };
}

function hasModifier(node, kind) {
  return node.modifiers?.some(modifier => modifier.kind === kind) ?? false;
}

function localModuleTarget(importer, specifier, axis, packageRoot) {
  if (!specifier.startsWith(".") && !specifier.startsWith("/")) return undefined;
  const base = specifier.startsWith("/") ? specifier : resolve(dirname(importer), specifier);
  const extension = extname(base);
  if (
    extension &&
    !(axis === "runtime"
      ? RUNTIME_EXTENSIONS
      : [...DECLARATION_EXTENSIONS, ...RUNTIME_EXTENSIONS]
    ).includes(extension)
  ) {
    return undefined;
  }
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
  const candidates = axis === "runtime"
    ? extension
      ? [base, ...sourceSubstitutions]
      : [base, ...RUNTIME_EXTENSIONS.map(value => `${base}${value}`), ...RUNTIME_EXTENSIONS.map(value => join(base, `index${value}`))]
    : extension
      ? [declarationCandidate(base), ...sourceSubstitutions].filter(Boolean)
      : [...DECLARATION_EXTENSIONS.map(value => `${base}${value}`), ...DECLARATION_EXTENSIONS.map(value => join(base, `index${value}`))];
  const selected = candidates.find(isFile);
  if (!selected) return undefined;
  packagePath(packageRoot, selected);
  return selected;
}

function localAssetTarget(importer, specifier, packageRoot) {
  if (!specifier.startsWith(".") && !specifier.startsWith("/")) return undefined;
  const path = specifier.startsWith("/") ? specifier : resolve(dirname(importer), specifier);
  if (!isFile(path)) return undefined;
  packagePath(packageRoot, path);
  const extension = extname(path);
  return RUNTIME_EXTENSIONS.includes(extension) || DECLARATION_EXTENSIONS.includes(extension)
    ? undefined
    : path;
}

function moduleDescription(path, axis, packageRoot, cache) {
  const key = `${axis}:${path}`;
  if (cache.has(key)) return cache.get(key);
  const { file, checker } = parseModule(path);
  const description = { direct: new Map(), stars: [], imports: new Map(), specifiers: [], file, checker };
  cache.set(key, description);
  for (const statement of file.statements) {
    if (ts.isImportDeclaration(statement) && ts.isStringLiteral(statement.moduleSpecifier)) {
      const target = localModuleTarget(path, statement.moduleSpecifier.text, axis, packageRoot);
      description.specifiers.push({
        text: statement.moduleSpecifier.text,
        target,
        asset: target ? undefined : localAssetTarget(path, statement.moduleSpecifier.text, packageRoot)
      });
      if (!target || !statement.importClause) continue;
      if (statement.importClause.name) {
        description.imports.set(statement.importClause.name.text, { file: target, name: "default" });
      }
      const bindings = statement.importClause.namedBindings;
      if (bindings && ts.isNamedImports(bindings)) {
        for (const element of bindings.elements) {
          description.imports.set(element.name.text, {
            file: target,
            name: element.propertyName?.text ?? element.name.text
          });
        }
      }
    }
    if (ts.isExportDeclaration(statement)) {
      const module = statement.moduleSpecifier;
      const target = module && ts.isStringLiteral(module)
        ? localModuleTarget(path, module.text, axis, packageRoot)
        : undefined;
      if (module && ts.isStringLiteral(module)) {
        description.specifiers.push({
          text: module.text,
          target,
          asset: target ? undefined : localAssetTarget(path, module.text, packageRoot)
        });
      }
      if (!statement.exportClause) {
        if (target) description.stars.push(target);
        continue;
      }
      if (ts.isNamespaceExport(statement.exportClause)) {
        if (target) description.direct.set(statement.exportClause.name.text, { file: target, name: "*" });
        continue;
      }
      for (const element of statement.exportClause.elements) {
        const publicName = element.name.text;
        const localName = element.propertyName?.text ?? publicName;
        if (target) description.direct.set(publicName, { file: target, name: localName });
        else description.direct.set(publicName, description.imports.get(localName) ?? { file: path, name: localName });
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
    if ((ts.isFunctionDeclaration(statement) || ts.isClassDeclaration(statement)) && statement.name) {
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
        (ts.isIdentifier(node.expression) && node.expression.text === "require")) &&
      node.arguments.length === 1 &&
      ts.isStringLiteralLike(node.arguments[0])
    ) {
      const text = node.arguments[0].text;
      const opaque = text.endsWith(".wasm") || text.endsWith(".node");
      description.specifiers.push({
        text,
        target: opaque ? undefined : localModuleTarget(path, text, axis, packageRoot),
        asset: opaque ? undefined : localAssetTarget(path, text, packageRoot),
        dynamic: node.expression.kind === ts.SyntaxKind.ImportKeyword
      });
    }
    ts.forEachChild(node, visitDynamic);
  };
  visitDynamic(file);
  return description;
}

function bindExport(path, name, axis, packageRoot, cache, visiting = new Set()) {
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
    const result = bindExport(direct.file, direct.name, axis, packageRoot, cache, visiting);
    visiting.delete(identity);
    return result;
  }
  if (name === "default") {
    visiting.delete(identity);
    return undefined;
  }
  const candidates = description.stars
    .map(target => bindExport(target, name, axis, packageRoot, cache, visiting))
    .filter(Boolean);
  visiting.delete(identity);
  const unique = new Map(candidates.map(candidate => [`${candidate.module.digest}:${candidate.exportName}`, candidate]));
  if (unique.size > 1) fail("ambiguous-export", `export ${name} resolves through multiple star exports`);
  return unique.values().next().value;
}

function exportedNames(path, axis, packageRoot, cache, visiting = new Set()) {
  const identity = `${axis}:${path}`;
  if (visiting.has(identity)) return new Set();
  visiting.add(identity);
  const description = moduleDescription(path, axis, packageRoot, cache);
  const names = new Set(description.direct.keys());
  for (const target of description.stars) {
    for (const name of exportedNames(target, axis, packageRoot, cache, visiting)) {
      if (name !== "default") names.add(name);
    }
  }
  visiting.delete(identity);
  return names;
}

function exactExportBindings(runtime, declarations, packageRoot) {
  const cache = new Map();
  const runtimeNames = exportedNames(runtime.path, "runtime", packageRoot, cache);
  const declarationNames = exportedNames(declarations.path, "declarations", packageRoot, cache);
  const names = [...runtimeNames].filter(name => declarationNames.has(name)).sort();
  const exports = {};
  for (const name of names) {
    const runtimeTarget = bindExport(runtime.path, name, "runtime", packageRoot, cache);
    const declarationTarget = bindExport(declarations.path, name, "declarations", packageRoot, cache);
    if (!runtimeTarget || !declarationTarget) continue;
    exports[name] = { runtime: runtimeTarget, declarations: declarationTarget };
  }
  return { exports, cache };
}

function syntaxHazards(path, sourceFile, checker) {
  const hazards = [];
  const add = (kind, node) =>
    hazards.push({
      kind,
      source: `${path}:${node.getStart(sourceFile)}-${node.end}`,
      affectedExports: [],
      affectedDomains: [...DOMAIN_NAMES]
    });
  const visit = node => {
    if (ts.isCallExpression(node)) {
      if (node.expression.kind === ts.SyntaxKind.ImportKeyword) {
        if (node.arguments.length !== 1 || !ts.isStringLiteralLike(node.arguments[0])) {
          add("nonliteral-dynamic-loading", node);
        }
      } else if (ts.isIdentifier(node.expression) && node.expression.text === "eval") {
        add("eval", node);
      }
    }
    if (
      ts.isPropertyAccessExpression(node) &&
      ts.isIdentifier(node.expression) &&
      node.expression.text === "WebAssembly"
    ) {
      add("opaque-wasm", node);
    }
    if (
      ts.isBinaryExpression(node) &&
      ts.isIdentifier(node.left) &&
      !checker.getSymbolAtLocation(node.left) &&
      node.operatorToken.kind >= ts.SyntaxKind.FirstAssignment &&
      node.operatorToken.kind <= ts.SyntaxKind.LastAssignment
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
  acceptedDependencies = {}
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
    hazards.push(...syntaxHazards(relativePath, description.file, description.checker));
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
        visit(
          specifier.target,
          axis,
          specifier.dynamic && axis === "runtime" ? "literal-dynamic-chunk" : undefined
        );
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
}) {
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
  const { exports, cache } = exactExportBindings(runtime.file, declarations.file, logicalRoot);
  const closure = closureForRoots(
    logicalRoot,
    manifestPath,
    runtime.file,
    declarations.file,
    cache,
    acceptedDependencies
  );
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
    closure,
    exports,
    authority: "standalonePackageResolver"
  };
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
