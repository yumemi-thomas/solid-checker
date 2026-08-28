// Temporary-v2 package proposal producer.
//
// Node owns exact artifact acquisition and process/file lifecycle. Rust owns
// semantic inference, normalization, proposal closure weakening, compact
// encoding, and multi-artifact merging. This file never reads a summary.

import { randomUUID } from "node:crypto";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import process from "node:process";

import { runNativeAsync } from "../bin/launcher.mjs";
import { resolvePackageArtifacts } from "./artifact-resolution.mjs";

export const packageContractHelp = `Usage:
  solid-checker contract generate --integrity <SRI> [OPTIONS]

Generates an unaccepted temporary schema-version-2 proposal. Exact artifact
identity is acquired independently and Rust owns all semantic normalization.
The proposal does not become analyzer input until proof verification issues a
receipt for its exact bytes.

Options:
  --package-root <DIR>   Package root (default: current directory)
  --output <FILE>        Output path (default: solid-reactivity.json)
  --integrity <SRI>      Exact installed package tarball integrity (required)
  --entrypoint <SUBPATH> Exact exported subpath (repeatable; default: all finite subpaths)
  --conditions <LIST>    Exact runtime conditions, e.g. browser,development
  -h, --help             Show this help
`;

function parseArguments(arguments_) {
  const options = {
    packageRoot: process.cwd(),
    output: "",
    integrity: "",
    entrypoints: [],
    conditions: []
  };
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    if (["--help", "-h"].includes(argument)) return { ...options, help: true };
    const separator = argument.indexOf("=");
    const key = separator < 0 ? argument : argument.slice(0, separator);
    const value = separator < 0 ? arguments_[++index] : argument.slice(separator + 1);
    if (!key.startsWith("--") || value === undefined || value === "") {
      throw new Error(`${key} needs a value`);
    }
    if (key === "--package-root") options.packageRoot = value;
    else if (key === "--output") options.output = value;
    else if (key === "--integrity") options.integrity = value;
    else if (key === "--entrypoint") options.entrypoints.push(value);
    else if (key === "--conditions") {
      options.conditions.push(...value.split(",").map(item => item.trim()).filter(Boolean));
    } else {
      throw new Error(`unknown contract generation argument ${key}`);
    }
  }
  return options;
}

function finiteEntrypoints(manifest, requested) {
  if (requested.length) return [...new Set(requested)].sort();
  const exports_ = manifest.exports;
  if (!exports_ || typeof exports_ !== "object" || Array.isArray(exports_)) return ["."];
  const keys = Object.keys(exports_);
  const subpaths = keys.filter(key => key.startsWith("."));
  if (!subpaths.length) return ["."];
  const wildcard = subpaths.find(key => key.includes("*"));
  if (wildcard) {
    throw new Error(
      `package exports ${wildcard}; pass each finite --entrypoint explicitly so generation does not guess the public surface`
    );
  }
  return subpaths.sort();
}

function specifierFor(packageName, entrypoint) {
  if (entrypoint === ".") return packageName;
  if (!entrypoint.startsWith("./")) {
    throw new Error(`entrypoint ${JSON.stringify(entrypoint)} must be "." or start with "./"`);
  }
  return `${packageName}/${entrypoint.slice(2)}`;
}

function finiteConditionPartitions(manifest, requested) {
  if (requested.length) return [[...new Set(requested)].sort()];
  const conditions = new Set();
  const visit = value => {
    if (Array.isArray(value)) {
      for (const child of value) visit(child);
      return;
    }
    if (!value || typeof value !== "object") return;
    for (const [key, child] of Object.entries(value)) {
      if (
        !key.startsWith(".") &&
        !["default", "types", "import", "require", "node-addons"].includes(key)
      ) {
        conditions.add(key);
      }
      visit(child);
    }
  };
  visit(manifest.exports);
  const names = [...conditions].sort();
  if (names.length > 8) {
    throw new Error(
      `package exports contain ${names.length} independent conditions; pass an exact --conditions list because the finite partition would exceed 256 cases`
    );
  }
  return Array.from({ length: 2 ** names.length }, (_, mask) =>
    names.filter((_, index) => (mask & (1 << index)) !== 0)
  );
}

async function checked(args, cwd) {
  const child = await runNativeAsync("solid-checker", args, {
    cwd,
    env: { SOLID_CHECKER_DAEMON: "0" }
  });
  if (child.error) throw new Error(`could not start the native checker: ${child.error.message}`);
  if (child.status !== 0) {
    throw new Error(child.stderr.trim() || child.stdout.trim() || `native checker exited ${child.status}`);
  }
  return child;
}

function projectFiles(resolution) {
  const files = new Set([resolution.runtime.path]);
  for (const entry of resolution.closure.entries ?? []) {
    if (!["runtime", "literal-dynamic-chunk"].includes(entry.role)) continue;
    files.add(resolve(resolution.packageRoot, entry.path));
  }
  return [...files].sort();
}

async function analyzeArtifact({
  packageRoot,
  manifest,
  integrity,
  entrypoint,
  conditions,
  scratch
}) {
  const specifier = specifierFor(manifest.name, entrypoint);
  const importer = join(dirname(packageRoot), `.solid-checker-acquisition-${randomUUID()}.mjs`);
  const resolution = resolvePackageArtifacts({
    importer,
    specifier,
    packageRoot,
    conditions: [...new Set([...conditions, "import"])],
    resolutionKind: "import",
    integrity
  });
  const id = randomUUID();
  const project = join(scratch, `${id}-tsconfig.json`);
  const resolutionPath = join(scratch, `${id}-resolution.json`);
  const runtimeResolutions = join(scratch, `${id}-runtime-resolutions.json`);
  const output = join(scratch, `${id}-proposal.json`);
  const plan = join(scratch, `${id}-proposal-plan.json`);
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
        files: projectFiles(resolution)
      },
      null,
      2
    )}\n`
  );
  writeFileSync(resolutionPath, `${JSON.stringify(resolution, null, 2)}\n`);
  writeFileSync(runtimeResolutions, '{"schemaVersion":1,"resolutions":[]}\n');
  await checked(
    [
      "--project",
      project,
      "--emit-contract",
      output,
      "--emit-proposal-plan",
      plan,
      "--runtime-module-resolutions",
      runtimeResolutions,
      "--contract-resolution",
      resolutionPath,
      "--package-name",
      manifest.name,
      "--package-version",
      manifest.version,
      "--contract-entry-file",
      resolution.runtime.path,
      "--contract-package-root",
      packageRoot,
      "--runtime-conditions",
      [...new Set([...conditions, "import"])].sort().join(",")
    ],
    packageRoot
  );
  return {
    document: output,
    plan,
    identity: JSON.stringify({
      entrypoint,
      runtime: resolution.runtime,
      declarations: resolution.declarations,
      runtimeTrace: resolution.runtimeTrace,
      declarationTrace: resolution.declarationTrace,
      closure: resolution.closure.digest
    })
  };
}

export async function generatePackageContract(arguments_, { quiet = false } = {}) {
  const options = parseArguments(arguments_);
  if (options.help) {
    process.stdout.write(packageContractHelp);
    return;
  }
  if (!options.integrity) {
    throw new Error(
      "--integrity is required; exact package identity cannot be inferred from package.json"
    );
  }
  const packageRoot = resolve(options.packageRoot);
  const manifestPath = join(packageRoot, "package.json");
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  if (!manifest.name || !manifest.version) {
    throw new Error(`${manifestPath} must declare exact name and version`);
  }
  const entrypoints = finiteEntrypoints(manifest, options.entrypoints);
  const output = resolve(options.output || join(packageRoot, "solid-reactivity.json"));
  const scratch = mkdtempSync(join(tmpdir(), "solid-checker-contract-v2-"));
  const proposals = [];
  try {
    const seenCases = new Set();
    const partitions = finiteConditionPartitions(manifest, options.conditions);
    for (const entrypoint of entrypoints) {
      for (const conditions of partitions) {
        const proposal = await analyzeArtifact({
          packageRoot,
          manifest,
          integrity: options.integrity,
          entrypoint,
          conditions,
          scratch
        });
        if (seenCases.has(proposal.identity)) continue;
        seenCases.add(proposal.identity);
        proposals.push(proposal);
      }
    }
    mkdirSync(dirname(output), { recursive: true });
    await checked(
      [
        ...proposals.flatMap(proposal => ["--merge-contract", proposal.document]),
        "--merge-contract-output",
        output,
        ...proposals.flatMap(proposal => ["--merge-proposal-plan", proposal.plan]),
        "--merge-proposal-plan-output",
        `${output}.proposal.json`
      ],
      packageRoot
    );
    await checked(["--validate-contract", output], packageRoot);
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
  const result = {
    package: manifest.name,
    version: manifest.version,
    output,
    plan: `${output}.proposal.json`,
    schemaVersion: 2,
    entrypoints: entrypoints.length,
    artifactCases: proposals.length,
    accepted: false
  };
  if (!quiet) {
    process.stdout.write(
      `generated unaccepted temporary-v2 proposal for ${manifest.name}@${manifest.version} at ${output}; proof verification must issue its receipt\n`
    );
  }
  return result;
}
