#!/usr/bin/env bun
// Random-edit differential harness for the retained (daemon) IR.
//
// A daemon that keeps the Reactive IR between edits is correct only if, after
// any edit, its answer equals a fresh one-shot run's answer. This applies a
// seeded sequence of edits to a project and, after each one, compares the two
// byte for byte (exit status and stdout). With shadow mode on in the daemon
// (`SOLID_CHECKER_IR_SHADOW`, solid-reactive-ir/src/shadow.rs) it also fails
// on any report the daemon logged: shadow mode names the first differing IR
// table and row, and serves the fresh result, so output equality alone could
// hide a stale cache.
//
//   bun scripts/ir-edit-fuzz.mjs --fixtures [--steps 6] [--seed 1]
//       The short budget `make verify` runs: each listed fixture is copied to
//       a temp directory (never edited in place), edited and compared.
//   bun scripts/ir-edit-fuzz.mjs --project <tsconfig.json> --steps 30 --seed 7
//       The long manual budget, on any project (the rc17 corpus: app-game
//       apps/web, solid-groove, compass-ui apps/ui). Edits happen in place.
//       Every edited file is restored byte-exact at the end, on failure, and on
//       SIGINT/SIGTERM; a journal of the originals survives a SIGKILL:
//   bun scripts/ir-edit-fuzz.mjs --restore <journal-dir>
//
// Options: --checker <bin> (SOLID_CHECKER_BIN, else the debug build),
// --typefacts <bin> (SOLID_TYPEFACTS_BIN), --kinds a,b,c (default all),
// --keep-going (do not stop at the first mismatch), --no-shadow.
import { spawn } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  realpathSync,
  rmSync,
  statSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, relative, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { gateConcurrency, mapPool } from "./lib/pool.mjs";

export const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

/**
 * Fixture projects the short budget edits. Each has several source files with
 * a real cross-file relation (calls, imports, hand-outs), no runtime flags,
 * and no `variants/`, so a copy of the directory is the whole project.
 */
export const FIXTURE_PROJECTS = [
  "reactive-ir/callback-forwarding",
  "reactive-ir/accessor-provenance",
  "reactive-ir/async-boundary",
  "reactive-ir/deferred-effect-apply",
  "reactive-ir/call-valued-callee-identity",
  "reactive-ir/callback-props-reads"
];

/** Mulberry32: a seeded generator, so a failing run replays from its seed. */
export function seededRandom(seed) {
  let state = seed >>> 0;
  const next = () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  return (n) => Math.floor(next() * n);
}

const isJsx = (path) => /\.[jt]sx$/.test(path);
const importLine = /^import\b[^\n]*\bfrom\s+["'][^"']+["'];?[ \t]*$/m;

/**
 * The edit kinds. Each takes `{ text, path, others, rand, serial }` and returns
 * the new text, or `null` when the file has no site for the edit (the step
 * then picks another kind). An edit may leave a file unparseable or ill-typed:
 * both paths must agree on that too.
 *
 * `body` keeps every byte offset (a same-length change), `prefix` shifts every
 * span of the file, `delete-line` removes one line.
 */
export const EDIT_KINDS = {
  body(ctx) {
    const { text, rand } = ctx;
    const literals = [...text.matchAll(/"[^"\n]{2,}"|'[^'\n]{2,}'|\b\d{2,}\b/g)];
    if (literals.length === 0) return null;
    const hit = literals[rand(literals.length)];
    const old = hit[0];
    const quote = /^["']/.test(old) ? old[0] : "";
    const inner = quote ? old.slice(1, -1) : old;
    const swapped = quote
      ? inner.replace(/./, (c) => (c === "x" ? "y" : "x"))
      : String(Number(inner) + 1).padStart(inner.length, "0").slice(-inner.length);
    if (swapped === inner) return null;
    return text.slice(0, hit.index) + quote + swapped + quote + text.slice(hit.index + old.length);
  },
  prefix: ({ text, rand }) => `// fuzz ${rand(1_000_000)}\n${text}`,
  "delete-line"({ text, rand }) {
    const lines = text.split("\n");
    const candidates = lines.flatMap((line, index) => (line.trim() === "" ? [] : [index]));
    if (candidates.length === 0) return null;
    lines.splice(candidates[rand(candidates.length)], 1);
    return lines.join("\n");
  },
  "add-import"({ text, path, others, rand, serial }) {
    if (others.length === 0) return null;
    const target = others[rand(others.length)];
    let specifier = relative(dirname(path), target).replace(/\.(tsx?|jsx?)$/, "");
    if (!specifier.startsWith(".")) specifier = `./${specifier}`;
    const line =
      rand(2) === 0
        ? `import * as __fz${serial} from "${specifier}";\nvoid __fz${serial};\n`
        : `import "${specifier}";\n`;
    return line + text;
  },
  "remove-import"({ text, rand }) {
    const lines = text.split("\n");
    const imports = lines.flatMap((line, index) => (importLine.test(line) ? [index] : []));
    if (imports.length === 0) return null;
    lines.splice(imports[rand(imports.length)], 1);
    return lines.join("\n");
  },
  "rename-export"({ text, serial }) {
    const found = /\bexport\s+(?:async\s+)?(?:const|let|function\*?|class)\s+([A-Za-z_$][\w$]*)/.exec(text);
    if (!found) return null;
    const name = found[1];
    return text.replace(new RegExp(`\\b${name.replace(/\$/g, "\\$")}\\b`), `${name}_fz${serial}`);
  },
  "add-component"({ text, path, serial }) {
    return isJsx(path)
      ? `${text}\nexport function FzComp${serial}(props: { v: number }) {\n  return <div>{props.v}</div>;\n}\n`
      : `${text}\nexport function fzHelper${serial}(value: number) {\n  return value + 1;\n}\n`;
  },
  "add-eval": ({ text, serial }) => `${text}\nexport const __fzEval${serial} = (code: string) => eval(code);\n`,
  "add-prototype-write": ({ text, path, serial }) =>
    `${text}\n${/\.tsx?$/.test(path) ? "(Object.prototype as any)" : "Object.prototype"}.__fz${serial} = 1;\n`
};

const LOADERS = { ".ts": "ts", ".tsx": "tsx", ".js": "js", ".jsx": "jsx", ".mts": "ts", ".cts": "ts", ".mjs": "js", ".cjs": "js" };

/**
 * Whether `text` parses as the language of `path`. The checker refuses an
 * unparseable project outright (exit 2, no answer), which says nothing about
 * the IR, so edits that break syntax are not applied. Without Bun's
 * transpiler every edit is assumed to parse and a refusal is caught below.
 */
export function parses(path, text) {
  const loader = LOADERS[path.slice(path.lastIndexOf("."))];
  if (!loader || typeof Bun === "undefined") return true;
  try {
    new Bun.Transpiler({ loader }).transformSync(text);
    return true;
  } catch {
    return false;
  }
}

/** Source files a project edit may touch, in a stable order. */
export function projectFiles(root) {
  const found = [];
  const walk = (directory) => {
    for (const name of readdirSync(directory)) {
      if (name.startsWith(".") || ["node_modules", "dist", "build", "target"].includes(name)) continue;
      const path = join(directory, name);
      const stat = statSync(path);
      if (stat.isDirectory()) walk(path);
      else if (/\.(tsx?|jsx?)$/.test(name) && !name.endsWith(".d.ts")) found.push(path);
    }
  };
  walk(root);
  return found.sort();
}

/**
 * Remembers the first content of every file it is asked to protect, journals it
 * to disk so a SIGKILL is recoverable, and puts everything back byte-exact.
 */
export class Restorer {
  constructor(journal) {
    this.journal = journal;
    this.originals = new Map();
    mkdirSync(journal, { recursive: true });
  }

  /** Call before the first write to `path`. */
  protect(path) {
    if (this.originals.has(path)) return;
    const bytes = readFileSync(path);
    const slot = join(this.journal, `${this.originals.size}.bin`);
    writeFileSync(slot, bytes);
    this.originals.set(path, bytes);
    writeFileSync(
      join(this.journal, "index.json"),
      JSON.stringify([...this.originals.keys()].map((original, index) => ({ path: original, slot: `${index}.bin` })))
    );
  }

  restore() {
    const problems = [];
    for (const [path, bytes] of this.originals) {
      writeFileSync(path, bytes);
      if (!readFileSync(path).equals(bytes)) problems.push(path);
    }
    return problems;
  }
}

/** Puts back the files of a journal left by a killed run. */
export function restoreJournal(journal) {
  const index = JSON.parse(readFileSync(join(journal, "index.json"), "utf8"));
  for (const { path, slot } of index) writeFileSync(path, readFileSync(join(journal, slot)));
  return index.length;
}

/** A child process run to completion without blocking the event loop. */
function run(command, args, options) {
  return new Promise((resolveRun) => {
    const child = spawn(command, args, { ...options, stdio: ["ignore", "pipe", "pipe"] });
    const out = [];
    const err = [];
    child.stdout.on("data", (chunk) => out.push(chunk));
    child.stderr.on("data", (chunk) => err.push(chunk));
    child.on("error", (error) => resolveRun({ status: null, stdout: "", stderr: String(error) }));
    child.on("close", (status) =>
      resolveRun({
        status,
        stdout: Buffer.concat(out).toString("utf8"),
        stderr: Buffer.concat(err).toString("utf8")
      })
    );
  });
}

function parseArguments(argv) {
  const options = { steps: 6, seed: 1, kinds: Object.keys(EDIT_KINDS), shadow: true };
  for (let index = 0; index < argv.length; index += 1) {
    const flag = argv[index];
    const value = () => {
      index += 1;
      if (argv[index] === undefined) throw new Error(`${flag} needs a value`);
      return argv[index];
    };
    if (flag === "--fixtures") options.fixtures = true;
    else if (flag === "--project") options.project = resolve(value());
    else if (flag === "--steps") options.steps = Number(value());
    else if (flag === "--seed") options.seed = Number(value());
    else if (flag === "--checker") options.checker = resolve(value());
    else if (flag === "--typefacts") options.typefacts = resolve(value());
    else if (flag === "--kinds") options.kinds = value().split(",");
    else if (flag === "--restore") options.restore = resolve(value());
    else if (flag === "--keep-going") options.keepGoing = true;
    else if (flag === "--no-shadow") options.shadow = false;
    else throw new Error(`unknown argument ${flag}`);
  }
  for (const kind of options.kinds) {
    if (!(kind in EDIT_KINDS)) throw new Error(`unknown edit kind ${kind}`);
  }
  if (!Number.isInteger(options.steps) || options.steps < 1) throw new Error("--steps must be a positive integer");
  return options;
}

function locate(variable, ...candidates) {
  return process.env[variable] ?? candidates.find((candidate) => existsSync(candidate)) ?? candidates[0];
}

/**
 * One project's run. Returns `{ steps, mismatches, restoreProblems, log }`.
 * `directory` is edited in place; the caller copies a fixture first.
 */
export async function fuzzProject({ tsconfig, steps, seed, kinds, checker, typefacts, shadow, keepGoing, label }) {
  const root = dirname(tsconfig);
  const files = projectFiles(root);
  const rand = seededRandom(seed);
  const scratch = mkdtempSync(join(tmpdir(), "ir-edit-fuzz-"));
  const restorer = new Restorer(join(scratch, "journal"));
  const shadowLog = join(scratch, "shadow.log");
  const environment = {
    ...process.env,
    SOLID_TYPEFACTS_BIN: typefacts,
    // The daemon is stopped by this harness, not by its idle or memory limits.
    SOLID_CHECKER_DAEMON_MAX_RSS_MB: "0",
    ...(shadow
      ? { SOLID_CHECKER_IR_SHADOW: "1", SOLID_CHECKER_IR_SHADOW_LOG: shadowLog }
      : { SOLID_CHECKER_IR_SHADOW: "0" })
  };
  const check = (daemon) =>
    run(checker, ["--project", tsconfig, "--format", "json"], {
      cwd: root,
      env: { ...environment, SOLID_CHECKER_DAEMON: daemon ? "1" : "0" }
    });
  // The daemon's command line carries the canonical project path (macOS
  // resolves /var to /private/var), so match that one too or it leaks.
  const canonical = realpathSync(tsconfig);
  const stopDaemon = async () => {
    for (const path of new Set([tsconfig, canonical])) {
      await run("pkill", ["-f", "--", `--serve --project ${path}`], {});
    }
  };
  const log = [];
  const report = (line) => {
    log.push(line);
    console.log(line);
  };
  let restoreProblems = [];
  const finish = async () => {
    await stopDaemon();
    restoreProblems = restorer.restore();
  };
  const onSignal = (signal) => {
    finish().then(() => process.exit(128 + (signal === "SIGINT" ? 2 : 15)));
  };
  process.once("SIGINT", onSignal);
  process.once("SIGTERM", onSignal);

  let mismatches = 0;
  let executed = 0;
  const retainedMs = [];
  const freshMs = [];
  try {
    await stopDaemon();
    await check(true); // The daemon's first (cold) build.
    for (let step = 0; step < steps; step += 1) {
      const path = files[rand(files.length)];
      restorer.protect(path);
      const current = readFileSync(path, "utf8");
      const original = restorer.originals.get(path).toString("utf8");
      let kind;
      let next = null;
      for (let attempt = 0; attempt < 12 && next === null; attempt += 1) {
        kind = rand(5) === 0 && current !== original ? "revert" : kinds[rand(kinds.length)];
        next =
          kind === "revert"
            ? original
            : EDIT_KINDS[kind]({
                text: current,
                path,
                others: files.filter((other) => other !== path),
                rand,
                serial: step
              });
        if (next !== null && !parses(path, next)) next = null;
      }
      if (next === null) continue;
      writeFileSync(path, next);
      executed += 1;
      const logSize = existsSync(shadowLog) ? statSync(shadowLog).size : 0;
      const started = performance.now();
      const retained = await check(true);
      const retainedTook = performance.now() - started;
      retainedMs.push(retainedTook);
      const freshStarted = performance.now();
      const fresh = await check(false);
      const freshTook = performance.now() - freshStarted;
      freshMs.push(freshTook);
      const shadowReport =
        existsSync(shadowLog) && statSync(shadowLog).size > logSize
          ? readFileSync(shadowLog, "utf8").slice(logSize)
          : "";
      // Two empty answers are equal and prove nothing: a checker that cannot
      // run the project must fail the run, not pass it. A refusal for syntax
      // the pre-check let through is skipped, and the file put back.
      const empty = fresh.stdout.trim() === "";
      if (empty && /parse failed/.test(fresh.stderr) && retained.status === fresh.status) {
        writeFileSync(path, current);
        report(`skip ${label} step ${step} ${relative(root, path)} ${kind} (refused: unparseable)`);
        continue;
      }
      const same = !empty && retained.status === fresh.status && retained.stdout === fresh.stdout;
      const name = `${label} step ${step} ${relative(root, path)} ${kind}`;
      if (same && !shadowReport) {
        report(
          `ok ${name} (daemon ${Math.round(retainedTook)} ms, one-shot ${Math.round(freshTook)} ms, ` +
            `${fresh.stdout.length} bytes)`
        );
        continue;
      }
      mismatches += 1;
      const out = join(scratch, `mismatch-${step}`);
      writeFileSync(`${out}.retained.json`, retained.stdout);
      writeFileSync(`${out}.fresh.json`, fresh.stdout);
      writeFileSync(`${out}.file`, next);
      writeFileSync(`${out}.retained.stderr`, retained.stderr);
      writeFileSync(`${out}.fresh.stderr`, fresh.stderr);
      report(
        `MISMATCH ${name} (status ${retained.status}/${fresh.status}) -> ${out}.*` +
          (shadowReport ? `\n${shadowReport.trim()}` : "")
      );
      if (!keepGoing) break;
    }
  } finally {
    await finish();
    process.removeListener("SIGINT", onSignal);
    process.removeListener("SIGTERM", onSignal);
  }
  const median = (values) => (values.length ? [...values].sort((a, b) => a - b)[values.length >> 1] : 0);
  report(
    `${label}: median daemon request ${Math.round(median(retainedMs))} ms, ` +
      `median one-shot ${Math.round(median(freshMs))} ms over ${retainedMs.length} step(s)`
  );
  if (restoreProblems.length > 0) report(`RESTORE FAILED: ${restoreProblems.join(", ")}`);
  if (mismatches === 0 && restoreProblems.length === 0) rmSync(scratch, { recursive: true, force: true });
  else report(`kept ${scratch}`);
  return { steps: executed, mismatches, restoreProblems, log };
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.restore) {
    console.log(`restored ${restoreJournal(options.restore)} file(s)`);
    return 0;
  }
  const checker = options.checker ?? locate("SOLID_CHECKER_BIN", join(ROOT, "rust", "target", "debug", "solid-checker-rust"));
  const typefacts = options.typefacts ?? locate("SOLID_TYPEFACTS_BIN", join(ROOT, "bin", "solid-typefacts"));
  for (const [name, path] of [
    ["checker", checker],
    ["type facts producer", typefacts]
  ]) {
    if (!existsSync(path)) throw new Error(`missing ${name} at ${path}`);
  }
  const shared = { ...options, checker, typefacts };
  let total = 0;
  let mismatches = 0;
  if (options.fixtures) {
    const results = await mapPool(FIXTURE_PROJECTS, async (id, index) => {
      // A copy: the fixture tree is never edited in place.
      const copy = mkdtempSync(join(tmpdir(), "ir-edit-fuzz-fixture-"));
      try {
        const directory = join(copy, basename(id));
        cpSync(join(ROOT, "fixtures", id), directory, { recursive: true, dereference: true });
        return await fuzzProject({ ...shared, tsconfig: join(directory, "tsconfig.json"), seed: options.seed + index, label: id });
      } finally {
        rmSync(copy, { recursive: true, force: true });
      }
    }, { concurrency: gateConcurrency() });
    for (const result of results) {
      total += result.steps;
      mismatches += result.mismatches + result.restoreProblems.length;
    }
  } else if (options.project) {
    const result = await fuzzProject({ ...shared, tsconfig: options.project, label: basename(dirname(options.project)) });
    total = result.steps;
    mismatches = result.mismatches + result.restoreProblems.length;
  } else {
    throw new Error("name --fixtures or --project <tsconfig.json>");
  }
  console.log(`ir-edit-fuzz: ${total} step(s), ${mismatches} mismatch(es)`);
  return mismatches === 0 ? 0 : 1;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().then(
    (code) => process.exit(code),
    (error) => {
      console.error(error.message);
      process.exit(2);
    }
  );
}
