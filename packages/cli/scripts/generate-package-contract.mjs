import { randomUUID } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, extname, join, relative, resolve, sep } from "node:path";
import process from "node:process";

import { runNative } from "../bin/launcher.mjs";
import { expandContract, normalizeContract } from "./contract-document.mjs";

export const packageContractHelp = `Usage:
  solid-checker contract generate [OPTIONS]

Options:
  --package-root <DIR>   Package root (default: current directory)
  --output <FILE>        Contract output path
  --entrypoint <SUBPATH> Generate one exact subpath (repeatable)
  --conditions <LIST>    Resolve conditional exports, e.g. browser,import
  --contract <FILE>      Dependency contract (repeatable)
  -h, --help             Show this help
`;

function parseArguments(arguments_) {
  const options = {
    packageRoot: process.cwd(),
    output: "",
    entrypoints: [],
    contracts: [],
    conditions: []
  };
  for (let index = 0; index < arguments_.length; index++) {
    const argument = arguments_[index];
    const separator = argument.indexOf("=");
    const key = separator === -1 ? argument : argument.slice(0, separator);
    const inline = separator === -1 ? undefined : argument.slice(separator + 1);
    const value = inline ?? arguments_[++index];
    if (!argument.startsWith("--") || value === undefined) {
      throw new Error(
        "usage: solid-checker contract generate [--package-root DIR] [--output FILE] " +
          "[--entrypoint SUBPATH] [--conditions LIST] [--contract FILE]"
      );
    }
    switch (key) {
      case "--package-root":
        options.packageRoot = value;
        break;
      case "--output":
        options.output = value;
        break;
      case "--entrypoint":
        options.entrypoints.push(value);
        break;
      case "--contract":
        options.contracts.push(value);
        break;
      case "--conditions":
        options.conditions.push(
          ...value
            .split(",")
            .map(condition => condition.trim())
            .filter(Boolean)
        );
        break;
      default:
        throw new Error(`unknown contract generation argument ${key}`);
    }
  }
  return options;
}

function runtimeLeaf(target) {
  if (typeof target !== "string" || target.endsWith(".d.ts")) return false;
  return [".js", ".jsx", ".mjs", ".ts", ".tsx", ".mts"].includes(extname(target));
}

function collectRuntimeLeaves(target, conditions = []) {
  if (typeof target === "string") {
    return runtimeLeaf(target) ? [{ target, conditions }] : [];
  }
  if (Array.isArray(target)) {
    return target.flatMap(value => collectRuntimeLeaves(value, conditions));
  }
  if (!target || typeof target !== "object") return [];
  return Object.entries(target).flatMap(([condition, value]) => {
    if (condition === "types" || condition === "require") return [];
    return collectRuntimeLeaves(
      value,
      condition === "default" ? conditions : [...conditions, condition]
    );
  });
}

function stringTargets(target) {
  if (typeof target === "string") return [target];
  if (Array.isArray(target)) return target.flatMap(stringTargets);
  if (!target || typeof target !== "object") return [];
  return Object.values(target).flatMap(stringTargets);
}

function resolveRuntimeLeaf(target, active, conditions = []) {
  if (typeof target === "string") {
    return runtimeLeaf(target) ? [{ target, conditions }] : [];
  }
  if (Array.isArray(target)) {
    for (const value of target) {
      const resolved = resolveRuntimeLeaf(value, active, conditions);
      if (resolved.length) return resolved;
    }
    return [];
  }
  if (!target || typeof target !== "object") return [];
  for (const [condition, value] of Object.entries(target)) {
    if (condition === "types" || condition === "require") continue;
    if (condition === "default" || active.has(condition)) {
      const resolved = resolveRuntimeLeaf(
        value,
        active,
        condition === "default" ? conditions : [...conditions, condition]
      );
      if (resolved.length) return resolved;
    }
  }
  return [];
}

function walkFiles(directory, root = directory, files = []) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (
      entry.name === "node_modules" ||
      entry.name === ".git" ||
      entry.name.startsWith(".solid-checker-contract-")
    ) {
      continue;
    }
    const path = join(directory, entry.name);
    if (entry.isDirectory()) walkFiles(path, root, files);
    else if (entry.isFile()) files.push(`./${relative(root, path).replaceAll(sep, "/")}`);
  }
  return files;
}

function hasRelativeModuleReference(path) {
  const source = readFileSync(path, "utf8")
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .split("\n")
    .filter(line => !line.trimStart().startsWith("//"))
    .join("\n");
  return /(?:\bfrom\s*|\bimport\s*\(\s*|\bimport\s*)["']\.\.?\//.test(source);
}

function patternCapture(pattern, candidate) {
  const star = pattern.indexOf("*");
  if (star === -1) return pattern === candidate ? "" : undefined;
  if (pattern.indexOf("*", star + 1) !== -1) {
    throw new Error(`package export pattern may contain only one wildcard: ${pattern}`);
  }
  const prefix = pattern.slice(0, star);
  const suffix = pattern.slice(star + 1);
  if (
    !candidate.startsWith(prefix) ||
    !candidate.endsWith(suffix) ||
    candidate.length < prefix.length + suffix.length
  ) {
    return undefined;
  }
  return candidate.slice(prefix.length, candidate.length - suffix.length);
}

function substituteStar(pattern, capture) {
  const star = pattern.indexOf("*");
  if (star === -1 || pattern.indexOf("*", star + 1) !== -1) {
    throw new Error(`package export pattern must contain one wildcard: ${pattern}`);
  }
  return `${pattern.slice(0, star)}${capture}${pattern.slice(star + 1)}`;
}

function packageTargetPath(packageRoot, target) {
  if (!target.startsWith("./")) {
    throw new Error(`package export target must be relative: ${target}`);
  }
  const path = resolve(packageRoot, target);
  if (path !== packageRoot && !path.startsWith(`${packageRoot}${sep}`)) {
    throw new Error(`package export target escapes the package root: ${target}`);
  }
  return path;
}

function existingPackageTarget(packageRoot, target) {
  const path = packageTargetPath(packageRoot, target);
  return existsSync(path) && statSync(path).isFile();
}

function concreteEntrypoints(packageRoot, exports_, selectedConditions) {
  const map =
    typeof exports_ === "object" &&
    !Array.isArray(exports_) &&
    exports_ !== null &&
    Object.keys(exports_).some(key => key.startsWith("."))
      ? exports_
      : { ".": exports_ };
  const packageFiles = Object.keys(map).some(key => key.includes("*"))
    ? walkFiles(packageRoot)
    : [];
  const concrete = new Map();
  const add = (entrypoint, leaf) => {
    const item = concrete.get(entrypoint) ?? [];
    if (
      !item.some(
        existing =>
          existing.target === leaf.target &&
          JSON.stringify(existing.conditions) === JSON.stringify(leaf.conditions)
      )
    ) {
      item.push(leaf);
    }
    concrete.set(entrypoint, item);
  };

  for (const [entrypoint, target] of Object.entries(map)) {
    const leaves = selectedConditions.length
      ? resolveRuntimeLeaf(target, new Set(selectedConditions))
      : collectRuntimeLeaves(target);
    if (
      leaves.length === 0 &&
      stringTargets(target).some(target =>
        [".cjs", ".cts"].includes(extname(target))
      )
    ) {
      throw new Error(
        `${entrypoint} has only a CJS runtime target; CJS contract generation is unsupported`
      );
    }
    if (!entrypoint.includes("*")) {
      // A source checkout can legitimately advertise build-condition targets
      // that do not exist until packaging. Analyze every currently materialized
      // runtime variant; a package with no materialized variant still fails
      // below instead of silently receiving an empty contract.
      for (const leaf of leaves) {
        if (existingPackageTarget(packageRoot, leaf.target)) add(entrypoint, leaf);
      }
      continue;
    }
    for (const leaf of leaves) {
      if (!leaf.target.includes("*")) {
        throw new Error(
          `package export pattern ${entrypoint} has non-pattern target ${leaf.target}`
        );
      }
      for (const file of packageFiles) {
        const capture = patternCapture(leaf.target, file);
        if (capture === undefined) continue;
        add(substituteStar(entrypoint, capture), {
          target: file,
          conditions: leaf.conditions
        });
      }
    }
  }
  return concrete;
}

function packageLocalTarget(packageRoot, target) {
  const path = packageTargetPath(packageRoot, target);
  if (!existsSync(path) || !statSync(path).isFile()) {
    throw new Error(`package export target does not exist: ${target}`);
  }
  return path;
}

function defaultOutput(packageRoot, packageName) {
  if (resolve(process.cwd()) === packageRoot) {
    return join(packageRoot, "solid-reactivity.json");
  }
  return join(
    process.cwd(),
    ".solid-checker",
    "contracts",
    ...packageName.split("/"),
    "solid-reactivity.json"
  );
}

function dependencyContracts(packageRoot, manifest) {
  const dependencies = new Set([
    ...Object.keys(manifest.dependencies ?? {}),
    ...Object.keys(manifest.optionalDependencies ?? {}),
    ...Object.keys(manifest.peerDependencies ?? {})
  ]);
  const contracts = [];
  for (const dependency of [...dependencies].sort()) {
    let directory = packageRoot;
    while (true) {
      const candidate = join(
        directory,
        "node_modules",
        ...dependency.split("/"),
        "solid-reactivity.json"
      );
      if (existsSync(candidate)) {
        contracts.push(candidate);
        break;
      }
      const parent = dirname(directory);
      if (parent === directory) break;
      directory = parent;
    }
  }
  return contracts;
}

function sameSummary(left, right) {
  return JSON.stringify(left) === JSON.stringify(right);
}

function mergeUnique(left = [], right = [], compare) {
  const values = new Map();
  for (const value of [...left, ...right]) values.set(JSON.stringify(value), value);
  return [...values.values()].sort(compare);
}

function mergeSummaries(left, right) {
  if (sameSummary(left, right)) return left;
  if (left.kind !== right.kind) {
    // A callable contract is only safe when every selected runtime target is
    // callable. `value` is the conservative cross-condition surface.
    return { kind: "value" };
  }
  if (
    (left.returns && right.returns && !sameSummary(left.returns, right.returns)) ||
    (left.asyncBehavior && right.asyncBehavior && left.asyncBehavior !== right.asyncBehavior)
  ) {
    return undefined;
  }
  const merged = { kind: left.kind };
  const returns = left.returns ?? right.returns;
  if (returns) merged.returns = returns;
  const callbacks = mergeUnique(
    left.callbacks,
    right.callbacks,
    (a, b) => a.parameter - b.parameter || a.execution.localeCompare(b.execution)
  );
  if (callbacks.length) merged.callbacks = callbacks;
  const asyncBehavior = left.asyncBehavior ?? right.asyncBehavior;
  if (asyncBehavior) merged.asyncBehavior = asyncBehavior;
  const reactiveReads = mergeUnique(
    left.reactiveReads,
    right.reactiveReads,
    (a, b) => a.kind.localeCompare(b.kind) || a.label.localeCompare(b.label)
  );
  if (reactiveReads.length) merged.reactiveReads = reactiveReads;
  return merged;
}

function runChecked(args, options = {}) {
  const result = runNative("solid-checker", args, {
    ...options,
    encoding: "utf8",
    stdio: "pipe"
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(
      [result.stderr, result.stdout]
        .filter(Boolean)
        .join("\n")
        .trim() || `native solid-checker exited ${result.status}`
    );
  }
  return result;
}

function analyzeTarget({
  packageRoot,
  packageName,
  packageVersion,
  target,
  contracts,
  temporaryDirectory,
  identifier,
  excludedTargets
}) {
  const entryFile = packageLocalTarget(packageRoot, target);
  const implementationRoot = dirname(entryFile);
  const excludedFiles = new Set(
    excludedTargets.map(target => packageLocalTarget(packageRoot, target))
  );
  const implementationFiles = hasRelativeModuleReference(entryFile)
    ? walkFiles(implementationRoot)
        .filter(runtimeLeaf)
        .map(file => resolve(implementationRoot, file))
        .filter(file => file === entryFile || !excludedFiles.has(file))
    : [entryFile];
  const project = join(temporaryDirectory, `${identifier}-tsconfig.json`);
  const output = join(temporaryDirectory, `${identifier}.json`);
  writeFileSync(
    project,
    `${JSON.stringify(
      {
        compilerOptions: {
          allowJs: true,
          checkJs: true,
          jsx: "preserve",
          module: "ESNext",
          moduleResolution: "Bundler",
          skipLibCheck: true,
          target: "ES2022"
        },
        // Runtime files are explicit roots because a published ESM barrel's
        // `.js` specifiers often resolve to adjacent `.d.ts` files when only
        // the entrypoint is seeded. The native emitter filters unresolved
        // behavior by this entrypoint's exact runtime identities, so private
        // siblings cannot poison its public contract.
        files: implementationFiles
      },
      null,
      2
    )}\n`
  );
  try {
    const args = [
      "--project",
      project,
      "--emit-contract",
      output,
      "--package-name",
      packageName,
      "--package-version",
      packageVersion,
      "--contract-entry-file",
      entryFile,
      "--contract-package-root",
      packageRoot
    ];
    for (const contract of contracts) args.push("--contract", resolve(contract));
    try {
      runChecked(args, { cwd: packageRoot });
    } catch (error) {
      if (error.message.includes("has no runtime ESM exports")) return {};
      throw error;
    }
    return expandContract(JSON.parse(readFileSync(output, "utf8"))).entrypoints["."].exports;
  } finally {
    rmSync(project, { force: true });
  }
}

export async function generatePackageContract(arguments_) {
  if (arguments_.includes("--help") || arguments_.includes("-h")) {
    process.stdout.write(packageContractHelp);
    return;
  }
  const options = parseArguments(arguments_);
  const packageRoot = resolve(options.packageRoot);
  const manifestPath = join(packageRoot, "package.json");
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  if (!manifest.name || !manifest.version || !manifest.exports) {
    throw new Error(
      `${manifestPath} must declare name, version, and exports for package contract generation`
    );
  }
  const discovered = concreteEntrypoints(
    packageRoot,
    manifest.exports,
    options.conditions
  );
  const selected = options.entrypoints.length
    ? new Map(
        options.entrypoints.map(entrypoint => {
          const variants = discovered.get(entrypoint);
          if (!variants) throw new Error(`package has no runtime entrypoint ${entrypoint}`);
          return [entrypoint, variants];
        })
      )
    : discovered;
  if (selected.size === 0) {
    throw new Error(`${manifest.name} has no supported ESM runtime entrypoints`);
  }

  const output = resolve(options.output || defaultOutput(packageRoot, manifest.name));
  const contracts = [
    ...new Set([
      ...options.contracts.map(contract => resolve(contract)),
      ...dependencyContracts(packageRoot, manifest)
    ])
  ];
  const temporaryDirectory = mkdtempSync(join(tmpdir(), "solid-checker-contract-"));
  const entrypoints = {};
  const targetsByEntrypoint = new Map();
  // Conditional exports frequently point several public entrypoints at the
  // same runtime target. Retain that target analysis for this generation
  // instead of rebuilding TypeScript, Reactive IR, and dependency contracts
  // once per public alias.
  const targetAnalyses = new Map();
  try {
    let ordinal = 0;
    for (const [entrypoint, variants] of [...selected].sort(([left], [right]) =>
      left.localeCompare(right)
    )) {
      const exports = {};
      const conditions = new Set();
      const targets = new Set();
      for (const variant of variants) {
        variant.conditions.forEach(condition => conditions.add(condition));
        targets.add(variant.target);
      }
      targetsByEntrypoint.set(entrypoint, targets);
      for (const target of targets) {
        const excludedTargets = [...targets]
          .filter(candidate => candidate !== target)
          .sort();
        const analysisKey = JSON.stringify([target, excludedTargets]);
        let observed = targetAnalyses.get(analysisKey);
        if (!observed) {
          observed = analyzeTarget({
            packageRoot,
            packageName: manifest.name,
            packageVersion: manifest.version,
            target,
            contracts,
            temporaryDirectory,
            identifier: `${ordinal++}-${randomUUID()}`,
            excludedTargets
          });
          targetAnalyses.set(analysisKey, observed);
        }
        for (const [name, summary] of Object.entries(observed)) {
          const merged = exports[name] ? mergeSummaries(exports[name], summary) : summary;
          if (!merged) {
            throw new Error(
              `${manifest.name} ${entrypoint}:${name} has incompatible semantics across conditional targets: ${JSON.stringify(exports[name])} versus ${JSON.stringify(summary)}`
            );
          }
          exports[name] = merged;
        }
      }
      if (Object.keys(exports).length === 0) {
        continue;
      }
      entrypoints[entrypoint] = {
        exports: Object.fromEntries(
          Object.entries(exports).sort(([left], [right]) => left.localeCompare(right))
        ),
        ...(conditions.size ? { conditions: [...conditions].sort() } : {})
      };
    }

    if (Object.keys(entrypoints).length === 0) {
      throw new Error(`${manifest.name} has no runtime ESM exports`);
    }
    for (const target of new Set([...targetsByEntrypoint.values()].flatMap(set => [...set]))) {
      const aliases = [...targetsByEntrypoint]
        .filter(([, targets]) => targets.has(target))
        .map(([entrypoint]) => entrypoint)
        .filter(entrypoint => entrypoints[entrypoint]);
      if (aliases.length < 2) continue;
      const names = new Set(
        aliases.flatMap(entrypoint => Object.keys(entrypoints[entrypoint].exports))
      );
      for (const name of names) {
        const shared = aliases
          .map(entrypoint => entrypoints[entrypoint].exports[name])
          .filter(Boolean);
        if (shared.length < 2) continue;
        let merged = shared[0];
        for (const summary of shared.slice(1)) {
          merged = mergeSummaries(merged, summary);
          if (!merged) break;
        }
        if (!merged) continue;
        for (const entrypoint of aliases) {
          if (entrypoints[entrypoint].exports[name]) {
            entrypoints[entrypoint].exports[name] = merged;
          }
        }
      }
    }

    const contract = {
      schemaVersion: 1,
      package: { name: manifest.name, version: manifest.version },
      compilerFactsProtocol: 1,
      artifacts: {},
      entrypoints,
      evidence: { kind: "inferred", generator: "solid-checker package generator" }
    };
    mkdirSync(dirname(output), { recursive: true });
    const candidate = `${output}.tmp-${randomUUID()}`;
    writeFileSync(candidate, `${JSON.stringify(normalizeContract(contract), null, 2)}\n`);
    try {
      runChecked(["--validate-contract", candidate]);
      renameSync(candidate, output);
    } finally {
      rmSync(candidate, { force: true });
    }
  } finally {
    rmSync(temporaryDirectory, { recursive: true, force: true });
  }
  process.stdout.write(
    `generated ${manifest.name}@${manifest.version} contract with ${Object.keys(entrypoints).length} entrypoints at ${output}\n`
  );
}
