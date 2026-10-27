// Type-checks a generated case against the published Solid typings with tsrs, the
// Rust port of TypeScript-Go that the project uses as its `tsc` (the tsc oracle's
// direction). A generated program that does not type-check proves nothing: the
// checker must stay silent on it (invariant 1), so the fuzzer discards it.
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";

/** The binary the owner builds; `--tsc <path>` / FUZZ_TSC override it. Never `tsc` from node_modules. */
export const DEFAULT_TSC = "/Users/thomas/Documents/Github/tsrs/target/release/tsrs";

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
export function typecheck(dir, { tsc = process.env.FUZZ_TSC ?? DEFAULT_TSC, timeoutMs = 60000 } = {}) {
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
