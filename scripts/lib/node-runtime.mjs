// The Node binary the fuzzer's runtimes run on.
//
// `process.execPath` is whatever runs the caller, and the repository's own
// tests run under bun (packages/cli's vitest). A runtime spawned from there
// ran on bun too, and bun is not the runtime the oracle measures: rc.17
// reported no UNTRACKED_READ_AFTER_AWAIT for a read reached after an `await`
// through a wrapper (scripts/fuzz/misuse-fire.test.mjs), where Node reports it
// every time. Under bun the PATH's `node` is used instead; under Node, the
// running binary.
import { execFileSync } from "node:child_process";

export const NODE = process.versions.bun ? "node" : process.execPath;

let version;
/** The version of `NODE` (under bun, the PATH's `node`, asked once). */
export function nodeVersion() {
  version ??= process.versions.bun ? execFileSync(NODE, ["--version"], { encoding: "utf8" }).trim() : process.version;
  return version;
}
