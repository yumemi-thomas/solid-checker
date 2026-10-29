// Type-checks a generated case against the published Solid typings with tsrs, the
// Rust port of TypeScript-Go that the project uses as its `tsc` (the tsc oracle's
// direction). A generated program that does not type-check proves nothing: the
// checker must stay silent on it (invariant 1), so the fuzzer discards it.
import { execFileSync, spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const TSRS_RELATIVE = "tsrs/target/release/tsrs";

/** A release build in a `tsrs` checkout beside this repository. Only the last resort of `resolveTsc`. Never `tsc` from node_modules. */
export const DEFAULT_TSC = resolve(fileURLToPath(import.meta.url), "../../../..", TSRS_RELATIVE);

/** The `tsrs` checkout beside the main working tree, when this is a linked worktree (a worktree outside the checkout's parent has no `../tsrs`). */
function besideMainCheckout() {
  try {
    const common = execFileSync("git", ["rev-parse", "--path-format=absolute", "--git-common-dir"], { cwd: dirname(fileURLToPath(import.meta.url)), encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();
    return resolve(common, "../..", TSRS_RELATIVE);
  } catch {
    return null;
  }
}

/**
 * The type checker's path: `--tsc`, then SOLID_CHECKER_TSRS_BIN, then FUZZ_TSC (the older name), then a `tsrs` checkout beside this
 * repository, then beside the main working tree this worktree was made from. When none exists the first candidate that was
 * named (or DEFAULT_TSC) comes back, and `requireTypeChecker` refuses it.
 */
export function resolveTsc(explicit, env = process.env) {
  const named = [explicit, env.SOLID_CHECKER_TSRS_BIN, env.FUZZ_TSC].find((value) => value);
  if (named) return resolve(named);
  if (existsSync(DEFAULT_TSC)) return DEFAULT_TSC;
  const beside = besideMainCheckout();
  return beside && existsSync(beside) ? beside : DEFAULT_TSC;
}

/** Stops a run (exit 2) whose type checker is missing: every program would otherwise come out a `type-error` and the run would pass with no candidates. */
export function requireTypeChecker(tsc) {
  if (!tsc || !existsSync(tsc)) {
    throw new Error(`type checker not found: ${tsc}. Build tsrs and pass --tsc <tsrs>, or set SOLID_CHECKER_TSRS_BIN (a run without a type checker proves nothing)`);
  }
}

/**
 * A run in which more than this share of programs fail to type-check tests nothing: it is refused (exit 2). A healthy sweep sees a
 * few percent (the generator is type-correct by construction); a missing or broken tsrs, or a wrong install, sees nearly all.
 */
export const TYPE_ERROR_REFUSAL_SHARE = 0.5;

/** The refusal message for `typeErrors` of `total` programs, or null when the run may stand. */
export function typeErrorRefusal(typeErrors, total, { first } = {}) {
  if (!total || typeErrors === 0) return null;
  if (typeErrors < total && typeErrors / total <= TYPE_ERROR_REFUSAL_SHARE) return null;
  return `invalid run: ${typeErrors} of ${total} programs failed to type-check (the limit is ${TYPE_ERROR_REFUSAL_SHARE * 100}%, and a run where all fail proves nothing)${first ? `; first: ${first}` : ""}`;
}

/**
 * A run whose programs the harness could not judge, more than half of them (a checker that would not start, a missing producer),
 * proves nothing either; the run refuses rather than reporting an empty matrix as clean.
 */
export function harnessRefusal(harness, total, { first } = {}) {
  if (!total || harness / total <= 0.5) return null;
  return `invalid run: the harness could not judge ${harness} of ${total} programs${first ? `; first: ${first}` : ""}`;
}

/** The oracle's compiler options (scripts/runtime-oracle.mjs), so a program checks as the oracle will compile it. */
export const COMPILER_OPTIONS = {
  target: "ES2022", module: "ESNext", moduleResolution: "Bundler", jsx: "preserve",
  jsxImportSource: "@solidjs/web", lib: ["ES2022", "DOM", "DOM.Iterable"], types: [],
  skipLibCheck: true, strict: true, noEmit: true,
};

export function tsconfigFor(files) {
  return `${JSON.stringify({ compilerOptions: COMPILER_OPTIONS, files }, null, 2)}\n`;
}

/**
 * Runs `<tsc> -p <dir>/tsconfig.json --noEmit` and returns `{ ok, errors }`, one string per diagnostic.
 * The directory must hold the tsconfig and be below a `node_modules` with solid-js and @solidjs/web.
 */
export function typecheck(dir, { tsc = resolveTsc(), timeoutMs = 60000 } = {}) {
  return new Promise((resolve) => {
    if (!existsSync(tsc)) { resolve({ ok: false, errors: [`type checker not found: ${tsc}`], missing: true }); return; }
    const child = spawn(tsc, ["-p", join(dir, "tsconfig.json"), "--noEmit"], { cwd: dir, stdio: ["ignore", "pipe", "pipe"] });
    let out = "";
    const timer = setTimeout(() => child.kill("SIGKILL"), timeoutMs);
    child.stdout.on("data", (chunk) => { out += chunk; });
    child.stderr.on("data", (chunk) => { out += chunk; });
    child.on("error", (error) => { clearTimeout(timer); resolve({ ok: false, errors: [String(error.message)], missing: true }); });
    child.on("close", (status) => {
      clearTimeout(timer);
      const errors = out.split("\n").filter((line) => /error TS\d+/.test(line));
      resolve({ ok: status === 0 && errors.length === 0, errors });
    });
  });
}
