// Stable package-contract proposal producer.
//
// Node owns exact artifact acquisition and process/file lifecycle. Rust owns
// semantic inference, normalization, proposal closure weakening, compact
// encoding, and multi-artifact merging. This file never reads a summary.

import { randomUUID } from "node:crypto";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
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

Generates an unaccepted stable schema-version-1 proposal. Exact artifact
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

function terminalExportTargets(value, targets = []) {
  if (typeof value === "string") targets.push(value);
  else if (Array.isArray(value)) {
    for (const child of value) terminalExportTargets(child, targets);
  } else if (value && typeof value === "object") {
    for (const child of Object.values(value)) terminalExportTargets(child, targets);
  }
  return targets;
}

function packageFiles(packageRoot, limit = 100_000) {
  const files = [];
  const visit = (directory, prefix) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (entry.name === "node_modules") continue;
      const relative = prefix ? `${prefix}/${entry.name}` : entry.name;
      if (entry.isDirectory()) visit(join(directory, entry.name), relative);
      else if (entry.isFile()) files.push(`./${relative}`);
      if (files.length > limit) {
        throw new Error(`package file census exceeds ${limit} entries`);
      }
    }
  };
  visit(packageRoot, "");
  return files.sort();
}

function expandWildcardEntrypoint(key, target, files) {
  if (
    (key.match(/\*/g)?.length ?? 0) !== 1 ||
    (target.match(/\*/g)?.length ?? 0) !== 1 ||
    !target.startsWith("./") ||
    target.includes("\\") ||
    target.split("/").includes("..")
  ) {
    return null;
  }
  const [prefix, suffix] = target.split("*");
  const matches = [];
  for (const file of files) {
    if (!file.startsWith(prefix) || !file.endsWith(suffix)) continue;
    const capture = file.slice(prefix.length, file.length - suffix.length);
    if (
      !capture ||
      capture.split("/").some(part => !part || [".", ".."].includes(part))
    ) {
      continue;
    }
    matches.push(key.replace("*", capture));
  }
  return matches;
}

export function finiteEntrypoints(manifest, requested, packageRoot = null) {
  if (requested.length) {
    return { entrypoints: [...new Set(requested)].sort(), wildcardRefusals: [] };
  }
  const exports_ = manifest.exports;
  if (!exports_ || typeof exports_ !== "object" || Array.isArray(exports_)) {
    return { entrypoints: ["."], wildcardRefusals: [] };
  }
  const keys = Object.keys(exports_);
  const subpaths = keys.filter(key => key.startsWith("."));
  if (!subpaths.length) return { entrypoints: ["."], wildcardRefusals: [] };
  const wildcardKeys = subpaths.filter(key => key.includes("*")).sort();
  const wildcardRefusals = [];
  const entrypoints = subpaths.filter(key => !key.includes("*"));
  const files = packageRoot && wildcardKeys.length ? packageFiles(packageRoot) : [];
  for (const key of wildcardKeys) {
    const targets = terminalExportTargets(exports_[key]);
    const expanded = packageRoot
      ? targets.map(target => expandWildcardEntrypoint(key, target, files))
      : [];
    if (
      expanded.length === 0 ||
      expanded.some(matches_ => matches_ === null || matches_.length === 0)
    ) {
      wildcardRefusals.push(key);
      continue;
    }
    entrypoints.push(...expanded.flat());
  }
  entrypoints.sort();
  if (!entrypoints.length) {
    throw new Error(
      `package exports ${wildcardRefusals.join(", ")}; pass each finite --entrypoint explicitly so generation does not guess the public surface`
    );
  }
  return { entrypoints: [...new Set(entrypoints)], wildcardRefusals };
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

function stableRefusalReason(error, { packageRoot, scratch }) {
  return (error?.message ?? String(error))
    .replaceAll(packageRoot, "<package-root>")
    .replaceAll(scratch, "<scratch>");
}

export async function retainIndependentlyMergeableProposals(proposals, attemptMerge) {
  let merged = null;
  let acceptedCount = 0;
  const rejected = [];
  for (const [index, candidate] of proposals.entries()) {
    try {
      merged = await attemptMerge(merged, candidate, index);
      acceptedCount += 1;
    } catch (error) {
      rejected.push({ candidate, error });
    }
  }
  return { merged, acceptedCount, rejected };
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
    entrypoint,
    conditions,
    resolution,
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
  const { entrypoints, wildcardRefusals } = finiteEntrypoints(
    manifest,
    options.entrypoints,
    packageRoot
  );
  const output = resolve(options.output || join(packageRoot, "solid-reactivity.json"));
  const scratch = mkdtempSync(join(tmpdir(), "solid-checker-contract-"));
  const proposals = [];
  let emittedArtifactCases = 0;
  let certificationProposals = [];
  const refusals = wildcardRefusals.map(entrypoint => ({
    entrypoint,
    conditions: null,
    stage: "entrypoint-census",
    reason: "wildcard export requires an explicit finite --entrypoint census"
  }));
  try {
    const seenCases = new Set();
    const partitions = finiteConditionPartitions(manifest, options.conditions);
    for (const entrypoint of entrypoints) {
      for (const conditions of partitions) {
        let proposal;
        try {
          proposal = await analyzeArtifact({
            packageRoot,
            manifest,
            integrity: options.integrity,
            entrypoint,
            conditions,
            scratch
          });
        } catch (error) {
          refusals.push({
            entrypoint,
            conditions,
            stage: "artifact-case",
            reason: stableRefusalReason(error, { packageRoot, scratch })
          });
          continue;
        }
        if (seenCases.has(proposal.identity)) continue;
        seenCases.add(proposal.identity);
        proposals.push(proposal);
      }
    }
    if (proposals.length === 0) {
      const first = refusals[0];
      throw new Error(
        `no certifiable artifact case; ${refusals.length} case(s) refused` +
          (first ? `; first refusal: ${first.entrypoint}: ${first.reason}` : "")
      );
    }
    mkdirSync(dirname(output), { recursive: true });
    const merge = async (inputs, document, plan) =>
      checked(
        [
          ...inputs.flatMap(proposal => ["--merge-contract", proposal.document]),
          "--merge-contract-output",
          document,
          ...inputs.flatMap(proposal => ["--merge-proposal-plan", proposal.plan]),
          "--merge-proposal-plan-output",
          plan
        ],
        packageRoot
      );
    try {
      await merge(proposals, output, `${output}.proposal.json`);
      emittedArtifactCases = proposals.length;
      certificationProposals = proposals;
    } catch {
      // One contradictory proposal must not erase unrelated exact cases. The
      // fallback greedily replays Rust's merge boundary and refuses only the
      // candidate that makes the normalized document/plan inconsistent.
      const fallback = await retainIndependentlyMergeableProposals(
        proposals,
        async (merged, candidate, index) => {
          const nextDocument = join(scratch, `merge-${index}.json`);
          const nextPlan = `${nextDocument}.proposal.json`;
          const inputs = merged ? [merged, candidate] : [candidate];
          const proposalsToMerge = inputs.flatMap(input => input.certificationProposals ?? [input]);
          await merge(proposalsToMerge, nextDocument, nextPlan);
          return {
            document: nextDocument,
            plan: nextPlan,
            certificationProposals: proposalsToMerge
          };
        }
      );
      for (const { candidate, error } of fallback.rejected) {
        refusals.push({
          entrypoint: candidate.entrypoint,
          conditions: candidate.conditions,
          stage: "proposal-merge",
          reason: stableRefusalReason(error, { packageRoot, scratch })
        });
      }
      if (!fallback.merged) throw new Error("no independently mergeable artifact case remains");
      emittedArtifactCases = fallback.acceptedCount;
      certificationProposals = fallback.merged.certificationProposals;
      copyFileSync(fallback.merged.document, output);
      copyFileSync(fallback.merged.plan, `${output}.proposal.json`);
    }
    await checked(["--validate-contract", output], packageRoot);
    writeFileSync(
      `${output}.refusals.json`,
      `${JSON.stringify(
        {
          format: "solid-checker-contract-proposal-refusals",
          refusalVersion: 1,
          package: { name: manifest.name, version: manifest.version },
          refusals
        },
        null,
        2
      )}\n`
    );
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
  const result = {
    package: manifest.name,
    version: manifest.version,
    output,
    plan: `${output}.proposal.json`,
    schemaVersion: 1,
    entrypoints: entrypoints.length,
    artifactCases: emittedArtifactCases,
    refusedArtifactCases: refusals.length,
    certificationInputs: certificationProposals.map(proposal => ({
      entrypoint: proposal.entrypoint,
      conditions: proposal.conditions,
      resolution: proposal.resolution
    })),
    accepted: false
  };
  if (!quiet) {
    process.stdout.write(
      `generated unaccepted stable contract proposal for ${manifest.name}@${manifest.version} at ${output}` +
        (refusals.length
          ? `; ${refusals.length} artifact case(s) refused and omitted`
          : "") +
        "; proof verification must issue its receipt\n"
    );
  }
  return result;
}
