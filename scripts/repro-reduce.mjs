#!/usr/bin/env node
// Reduces one uncertifiable result of a real project to the few lines that
// still produce it: the first step of adding a case to the reproduction set
// (ADR 0446; CONTRIBUTING.md, "Adding a repo to the reproduction set").
//
//   node scripts/repro-reduce.mjs --source <app root> --out <scratch dir>
//     --project <tsconfig directory, relative to the app> [--tsconfig tsconfig.json]
//     --file <path, relative to the project directory> [--file ...]
//     --rule SC2001 --family unplaced-write [--context <kind>] --marker <text on the result's line>
//     [--boundary open|closed] [--restrict] [--checker <bin>] [--typefacts <bin>]
//
// The app is mirrored into --out (every file outside node_modules copied, each
// node_modules rebuilt as links: a workspace link stays relative, so it points
// into the copy, anything else links to the installed bytes), and only the
// copy is edited. `--restrict` narrows the copy's tsconfig to the first file
// (TypeScript still pulls in what it imports) when the result survives that.
// Then each --file in turn loses chunks of lines (ddmin) for as long as the
// checker, run with the corpus configuration (browser, development, csr),
// reports in the first file an uncertifiable result with --rule, the message
// family and analysis context kind of scripts/uncertifiable-causes.mjs, on a
// line containing --marker, and TypeScript 7.1 (the tsc oracle's install,
// `make tsc-oracle-provision`) reports no more errors in the reduced files
// than at the start. The guard matters: without it a reduction drifts into
// code that no longer type-checks (a deleted declaration leaves an unresolved
// name), which is a different cause behind the same wording. 7.1 refuses
// options real projects still use (`baseUrl`, TS5102; an `ignoreDeprecations`
// it does not know, TS5103) and then checks no file at all, so the copy's
// tsconfig chain drops them first (`baseUrl` folded into `paths`), and a
// configuration error left after that stops the run rather than passing it.
//
// The reduced lines are a starting point, not a case: rewrite them into one
// short file of the set, check that its result and cause match the source's
// (`make repro-causes`, and --report on the sweep), and record the source.
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  cpSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  readlinkSync,
  realpathSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { contextKind, messageFamily } from "./uncertifiable-causes.mjs";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const typescript = createRequire(join(repo, "packages/cli/package.json"))("typescript");
const options = {
  source: null,
  out: null,
  project: ".",
  tsconfig: "tsconfig.json",
  file: [],
  rule: null,
  family: null,
  context: "",
  marker: null,
  boundary: null,
  restrict: false,
  checker: join(repo, "rust/target/release/solid-checker-rust"),
  typefacts: join(repo, "bin/solid-typefacts"),
};
const argv = process.argv.slice(2);
for (let index = 0; index < argv.length; index++) {
  const name = argv[index].replace(/^--/, "");
  if (name === "restrict") {
    options.restrict = true;
    continue;
  }
  if (!(name in options) || argv[index + 1] === undefined) throw new Error(`unknown or incomplete option ${argv[index]}`);
  if (name === "file") options.file.push(argv[++index]);
  else options[name] = argv[++index];
}
for (const name of ["source", "out", "rule", "family", "marker"]) if (!options[name]) throw new Error(`--${name} is required`);
if (!options.file.length) throw new Error("--file is required");
const tsc = join(repo, "rust/target/tsc-oracle/typescript/node_modules/.bin/tsc");
if (!existsSync(tsc)) throw new Error(`no TypeScript at ${tsc}: run make tsc-oracle-provision`);

const source = resolve(options.source);
const out = resolve(options.out);

/** Mirrors `from` into `to`: files copied writable, node_modules rebuilt as links. */
function mirror(from, to) {
  const inside = (path) => !relative(source, path).startsWith("..");
  const linkEntries = (directory, target) => {
    mkdirSync(target, { recursive: true });
    for (const entry of readdirSync(directory)) {
      const path = join(directory, entry);
      const stat = lstatSync(path);
      if (entry.startsWith("@") && stat.isDirectory()) linkEntries(path, join(target, entry));
      else if (stat.isSymbolicLink() && inside(realpathSync(path)) && !realpathSync(path).includes("/node_modules/")) {
        symlinkSync(readlinkSync(path), join(target, entry));
      } else symlinkSync(stat.isSymbolicLink() ? realpathSync(path) : path, join(target, entry));
    }
  };
  mkdirSync(to, { recursive: true });
  for (const entry of readdirSync(from)) {
    if (entry === ".git" || entry.endsWith(".tsbuildinfo")) continue;
    const path = join(from, entry);
    const target = join(to, entry);
    const stat = lstatSync(path);
    if (entry === "node_modules" && stat.isDirectory()) linkEntries(path, target);
    else if (stat.isSymbolicLink()) symlinkSync(readlinkSync(path), target);
    else if (stat.isDirectory()) mirror(path, target);
    else {
      cpSync(path, target);
      // A frozen corpus is read-only (its restorer runs `chmod -R a-w`).
      chmodSync(target, stat.mode | 0o200);
    }
  }
}

if (existsSync(out)) throw new Error(`${out} exists; reduce into a fresh directory`);
mirror(source, out);
const project = join(out, options.project);
const target = join(project, options.file[0]);
const tsconfig = join(project, options.tsconfig);

/** A tsconfig, read with TypeScript's own JSONC parser (comments, trailing commas). */
const readJsonc = (path) => {
  const { config, error } = typescript.parseConfigFileTextToJson(path, readFileSync(path, "utf8"));
  if (error) throw new Error(`${path}: ${typescript.flattenDiagnosticMessageText(error.messageText, "\n")}`);
  return config;
};

/**
 * Makes the copy's tsconfig chain one TypeScript 7.1 accepts: no
 * `ignoreDeprecations`, and `baseUrl` folded into `paths` (7.1 resolves
 * `paths` from the file that declares them). Only files inside the copy are
 * rewritten; an `extends` into a package is left alone.
 */
function normalizeForTypeScript7(path, seen = new Set()) {
  if (seen.has(path) || !existsSync(path) || relative(out, path).startsWith("..")) return;
  seen.add(path);
  const config = readJsonc(path);
  const compilerOptions = config.compilerOptions ?? {};
  delete compilerOptions.ignoreDeprecations;
  // Nothing here needs build info, and a frozen corpus's is not writable.
  delete compilerOptions.tsBuildInfoFile;
  if (compilerOptions.incremental) compilerOptions.incremental = false;
  if (compilerOptions.composite) compilerOptions.composite = false;
  if (compilerOptions.baseUrl !== undefined) {
    const base = compilerOptions.baseUrl.replace(/\/$/, "");
    for (const [key, targets] of Object.entries(compilerOptions.paths ?? {})) {
      compilerOptions.paths[key] = targets.map((one) => (one.startsWith("/") ? one : `${base}/${one}`.replace(/\/\.\//g, "/")));
    }
    delete compilerOptions.baseUrl;
  }
  config.compilerOptions = compilerOptions;
  writeFileSync(path, `${JSON.stringify(config, null, 2)}\n`);
  for (const parent of [config.extends ?? []].flat()) {
    if (parent.startsWith(".")) normalizeForTypeScript7(resolve(dirname(path), parent.endsWith(".json") ? parent : `${parent}.json`), seen);
  }
}
normalizeForTypeScript7(tsconfig);

const tscErrors = () => {
  const run = spawnSync(tsc, ["--noEmit", "--pretty", "false", "-p", tsconfig], { encoding: "utf8", maxBuffer: 1 << 30 });
  const lines = `${run.stdout}${run.stderr}`.split("\n");
  // A configuration error has no source location, or sits in a tsconfig.
  const configuration = lines.find((line) => /error TS5\d{3}:/.test(line) || (/error TS\d+:/.test(line) && /^[^(]*tsconfig[^(]*\.json\(|^error /.test(line)));
  if (configuration) throw new Error(`TypeScript 7.1 refuses the copy's configuration: ${configuration}`);
  // An unused declaration (noUnusedLocals and kin) is what deleting its use
  // leaves behind; it changes nothing the checker reads.
  const unused = /error TS(6133|6192|6196|6198):/;
  return lines.filter((line) => !unused.test(line) && options.file.some((file) => line.includes(`${file}(`))).length;
};
let baseline = null;
let runs = 0;
function holds() {
  runs++;
  if (baseline !== null && tscErrors() > baseline) return false;
  const run = spawnSync(
    options.checker,
    [
      "--format", "json", "--project", tsconfig,
      "--runtime-target", "browser", "--runtime-build", "development", "--rendering", "csr",
      ...(options.boundary ? ["--program-boundary", options.boundary] : []),
    ],
    { encoding: "utf8", maxBuffer: 1 << 30, env: { ...process.env, SOLID_CHECKER_DAEMON: "0", SOLID_TYPEFACTS_BIN: resolve(options.typefacts) } },
  );
  let payload;
  try {
    payload = JSON.parse(run.stdout);
  } catch {
    return false;
  }
  const lines = readFileSync(target, "utf8").split("\n");
  return payload.findings.some(
    (finding) =>
      finding.kind === "uncertifiable" &&
      finding.id === options.rule &&
      messageFamily(finding.message) === options.family &&
      contextKind(finding.analysisContext) === options.context &&
      finding.primaryLocation.path === target &&
      (lines[finding.primaryLocation.line - 1] ?? "").includes(options.marker),
  );
}

if (options.restrict) {
  const original = readFileSync(tsconfig, "utf8");
  const parsed = readJsonc(tsconfig);
  parsed.include = [options.file[0]];
  delete parsed.files;
  delete parsed.references;
  writeFileSync(tsconfig, `${JSON.stringify(parsed, null, 2)}\n`);
  if (!holds()) {
    console.error("restricting the tsconfig loses the result; reducing the whole project");
    writeFileSync(tsconfig, original);
  }
}
baseline = tscErrors();
if (!holds()) {
  console.error("the result does not hold in the copy before any edit");
  process.exit(1);
}
for (const file of options.file) {
  const path = join(project, file);
  let lines = readFileSync(path, "utf8").split("\n");
  let parts = 2;
  while (lines.length >= 2) {
    const size = Math.ceil(lines.length / parts);
    let removed = false;
    for (let start = 0; start < lines.length; start += size) {
      const candidate = [...lines.slice(0, start), ...lines.slice(start + size)];
      writeFileSync(path, candidate.join("\n"));
      if (holds()) {
        lines = candidate;
        parts = Math.max(parts - 1, 2);
        removed = true;
        break;
      }
    }
    if (removed) continue;
    writeFileSync(path, lines.join("\n"));
    if (size === 1) break;
    parts = Math.min(lines.length, parts * 2);
  }
  console.log(`${file}: ${lines.length} lines after ${runs} checker runs`);
}
for (const file of options.file) console.log(`\n// ${file}\n${readFileSync(join(project, file), "utf8")}`);
