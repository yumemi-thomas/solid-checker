// The fixture projects the checker's gates run, and the runtime arguments each declares.
//
// This is `fixtureProjects`, `reproProjects` and `runtimeArguments` of scripts/coverage.mjs, which keeps its own copy (a gate
// script is not a module, and editing it would select every gate). scripts/proof-gate.test.mjs holds the two lists to each other:
// coverage writes one snapshot per project, so the snapshot directory is the list.
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join, relative } from "node:path";

/** Every fixture project: a directory holding a tsconfig.json, its variants, and the reproduction set's projects. */
export function fixtureProjects(root) {
  const found = [];
  for (const group of ["reactive-ir", "engine"]) {
    const base = join(root, "fixtures", group);
    if (!existsSync(base)) continue;
    for (const entry of readdirSync(base, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      const directory = join(base, entry.name);
      const tsconfig = join(directory, "tsconfig.json");
      if (!existsSync(tsconfig)) continue;
      found.push({ id: `${group}/${entry.name}`, directory, tsconfig });
      // Variants of one fixture share its `node_modules` by Node's own lookup: each `variants/<name>/` is a project of its own.
      const variants = join(directory, "variants");
      if (existsSync(variants)) {
        for (const variant of readdirSync(variants, { withFileTypes: true })) {
          const variantTsconfig = join(variants, variant.name, "tsconfig.json");
          if (!variant.isDirectory() || !existsSync(variantTsconfig)) continue;
          found.push({
            id: `${group}/${entry.name}-${variant.name}`,
            directory: join(variants, variant.name),
            tsconfig: variantTsconfig,
          });
        }
      }
    }
  }
  found.push(...reproProjects(root));
  return found.sort((a, b) => a.id.localeCompare(b.id));
}

/** The uncertifiable reproduction set (ADR 0446): projects at any depth, whose id joins the path with `-`. */
function reproProjects(root) {
  const base = join(root, "fixtures", "uncertifiable-repro");
  if (!existsSync(base)) return [];
  const found = [];
  const walk = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (!entry.isDirectory() || entry.name === "node_modules" || entry.name.startsWith(".")) continue;
      const path = join(directory, entry.name);
      const tsconfig = join(path, "tsconfig.json");
      if (existsSync(tsconfig)) {
        found.push({
          id: `uncertifiable-repro/${relative(base, path).split("/").join("-")}`,
          directory: path,
          tsconfig,
        });
      }
      walk(path);
    }
  };
  walk(base);
  return found;
}

/** The checker arguments a project's `.solid-checker/runtime.json` declares. */
export function runtimeArguments(tsconfig) {
  const metadata = join(dirname(tsconfig), ".solid-checker", "runtime.json");
  if (!existsSync(metadata)) return [];
  const runtime = JSON.parse(readFileSync(metadata, "utf8"));
  if (runtime == null || typeof runtime !== "object" || Array.isArray(runtime)) {
    throw new Error(`${metadata}: runtime metadata must be an object`);
  }
  const args = [];
  for (const [key, flag] of [
    ["target", "--runtime-target"],
    ["build", "--runtime-build"],
    ["realm", "--runtime-realm"],
    ["rendering", "--rendering"],
    ["programBoundary", "--program-boundary"],
  ]) {
    if (runtime[key] !== undefined) args.push(flag, runtime[key]);
  }
  for (const condition of runtime.conditions ?? []) args.push("--runtime-condition", condition);
  for (const transform of runtime.frameworkTransforms ?? []) args.push("--framework-transform", transform);
  for (const specifier of runtime.dependencySources ?? []) args.push("--dependency-source", specifier);
  if (runtime.dependencySource !== undefined) {
    throw new Error(`${metadata}: dependencySource is retired with package contracts`);
  }
  return args;
}
