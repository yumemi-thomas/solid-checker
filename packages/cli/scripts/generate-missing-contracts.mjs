import { existsSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import process from "node:process";

import { runNative } from "../bin/launcher.mjs";
import {
  generatePackageContract,
  packageContractHelp
} from "./generate-package-contract.mjs";

// Every other `contract generate` option describes one package. Accepting one
// beside `--missing` would apply a package-scoped assertion -- an entrypoint,
// an output path, a condition selection -- to every package in the project.
const sweepOptions = new Set(["--project", "--format"]);

function parseSweepArguments(arguments_) {
  const options = { project: "", format: "text" };
  for (let index = 0; index < arguments_.length; index++) {
    const argument = arguments_[index];
    const separator = argument.indexOf("=");
    const key = separator === -1 ? argument : argument.slice(0, separator);
    const inline = separator === -1 ? undefined : argument.slice(separator + 1);
    if (key === "--missing") {
      if (inline !== undefined) throw new Error("--missing takes no value");
      continue;
    }
    if (!sweepOptions.has(key)) {
      throw new Error(
        `${key} generates one package and --missing sweeps every missing one; ` +
          "--missing takes only --project and --format"
      );
    }
    const value = inline ?? arguments_[++index];
    if (value === undefined) throw new Error(`${key} needs a value`);
    if (key === "--project") options.project = value;
    else options.format = value;
  }
  if (!["text", "json"].includes(options.format)) {
    throw new Error(`unsupported format ${options.format}`);
  }
  return options;
}

// Contract discovery walks the ancestors of the *project directory*, so both
// the package roots the sweep generates from and the project-owned outputs it
// writes are anchored there rather than at the working directory.
function projectDirectory(project) {
  const path = resolve(project || "tsconfig.json");
  if (existsSync(path) && statSync(path).isDirectory()) return path;
  return dirname(path);
}

function coverageReport(project) {
  const child = runNative(
    "solid-checker",
    ["--check-contracts", "--format", "json", ...(project ? ["--project", project] : [])],
    { stdio: ["ignore", "pipe", "pipe"], encoding: "utf8" }
  );
  if (child.error) throw new Error(`could not start the native checker: ${child.error.message}`);
  // The report exits 1 exactly when a package needs action, which is this
  // sweep's input rather than a failure of it.
  if (child.status !== 0 && child.status !== 1) {
    throw new Error(
      String(child.stderr ?? "").trim() || `native solid-checker exited ${child.status}`
    );
  }
  let report;
  try {
    report = JSON.parse(child.stdout);
  } catch (error) {
    throw new Error(`could not read the contract report: ${error.message}`);
  }
  return Array.isArray(report?.packages) ? report.packages : [];
}

function installedPackageRoot(directory, name) {
  let current = directory;
  for (;;) {
    const candidate = join(current, "node_modules", ...name.split("/"));
    if (existsSync(join(candidate, "package.json"))) return candidate;
    const parent = dirname(current);
    if (parent === current) return undefined;
    current = parent;
  }
}

function localContractPath(directory, name) {
  return join(directory, ".solid-checker", "contracts", ...name.split("/"), "solid-reactivity.json");
}

export async function generateMissingContracts(arguments_) {
  if (arguments_.includes("--help") || arguments_.includes("-h")) {
    process.stdout.write(packageContractHelp);
    return;
  }
  const options = parseSweepArguments(arguments_);
  const json = options.format === "json";
  const directory = projectDirectory(options.project);
  const packages = coverageReport(options.project);
  const missing = packages.filter(entry => entry.status === "missing");
  // A certifying status carries no remedy. Everything else that is not
  // `missing` -- an unverified draft, a contract that drifted off its artifact
  // -- already has a contract on disk, and regenerating it would clobber work
  // a reviewer owns.
  const skipped = packages.filter(entry => entry.status !== "missing" && entry.remedy);

  if (!json) {
    for (const entry of skipped) {
      process.stdout.write(`${entry.name}: ${entry.status}, left alone; ${entry.remedy}\n`);
    }
  }

  if (missing.length === 0) {
    if (json) {
      process.stdout.write(
        `${JSON.stringify({ generated: [], skipped, failed: [] }, null, 2)}\n`
      );
    } else {
      process.stdout.write("no package contract is missing; nothing to generate.\n");
    }
    return;
  }

  const generated = [];
  const failed = [];
  for (const entry of missing) {
    try {
      const packageRoot = installedPackageRoot(directory, entry.name);
      if (!packageRoot) throw new Error(`no installed package at node_modules/${entry.name}`);
      if (!entry.installedIntegrity) {
        throw new Error(
          "the package manager supplied no exact registry integrity; linked and local packages cannot be certified automatically"
        );
      }
      const result = await generatePackageContract(
        [
          "--package-root",
          packageRoot,
          "--output",
          localContractPath(directory, entry.name),
          "--integrity",
          entry.installedIntegrity
        ],
        { quiet: json }
      );
      generated.push(result);
    } catch (error) {
      // One package proves nothing about the next, so a failure here is
      // recorded and the sweep continues. The exit code below is what keeps it
      // from passing as a complete run.
      //
      // The report carries the whole message and stderr carries its first line.
      // A native panic's useful part -- the assertion, the location -- is on the
      // lines after the first, and truncating it in the machine-readable report
      // too left a CI run with no record of why a package failed and no way to
      // recover one.
      const reason = String(error?.message ?? error);
      failed.push({ package: entry.name, reason });
      if (!json) process.stderr.write(`solid-checker: ${entry.name}: ${reason.split("\n")[0]}\n`);
    }
  }

  if (json) {
    process.stdout.write(`${JSON.stringify({ generated, skipped, failed }, null, 2)}\n`);
  } else {
    const withRefusals = generated.filter(result => result.refusedEntrypoints > 0).length;
    process.stdout.write(
      `swept ${missing.length} missing package(s): ${generated.length - withRefusals} generated, ` +
        `${withRefusals} generated with refused entrypoints, ${failed.length} failed\n`
    );
  }
  if (failed.length) process.exitCode = 1;
}
