import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
import { certificationEnvironment } from "./lib/certification-environment.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));

export function focusedTestArguments({ TEST, TEST_EXACT = "0", TEST_PACKAGE = "solid-facts-backend" }) {
  if (!TEST?.trim() || TEST.startsWith("-")) throw new Error("set TEST to a nonempty Rust test name or filter");
  if (!["0", "1"].includes(TEST_EXACT)) throw new Error("TEST_EXACT must be 0 or 1");
  if (!TEST_PACKAGE || TEST_PACKAGE.startsWith("-")) throw new Error("TEST_PACKAGE must name a Rust package");
  return ["+1.97", "test", "--manifest-path", "rust/Cargo.toml", "-p", TEST_PACKAGE,
    "--lib", TEST, "--", ...(TEST_EXACT === "1" ? ["--exact"] : [])];
}

export function main(environment = process.env, spawn = spawnSync, prepareEnvironment = certificationEnvironment) {
  const args = focusedTestArguments(environment);
  const prepared = spawn("make", ["build-typefacts"], {
    cwd: root, env: environment, stdio: "inherit"
  });
  if (prepared.error || prepared.status !== 0) return prepared.status || 1;
  const env = prepareEnvironment(root, environment);
  // Compile and enumerate first. A misspelled filter must never be a green run.
  console.log(`Compiling and selecting tests matching ${environment.TEST}...`);
  const listed = spawn("cargo", [...args, "--list"], { cwd: root, env, encoding: "utf8" });
  if (listed.error || listed.status !== 0) {
    process.stderr.write(listed.stderr || String(listed.error || "test listing failed"));
    return listed.status || 1;
  }
  const names = listed.stdout.split("\n").filter(line => line.endsWith(": test"));
  if (!names.length) throw new Error(`TEST=${environment.TEST} matched no tests`);
  console.log(`Running ${names.length} focused test(s):\n${names.join("\n")}`);
  const result = spawn("cargo", args, { cwd: root, env, stdio: "inherit" });
  return result.status ?? 1;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { process.exitCode = main(); }
  catch (error) { console.error(`test-focused: ${error.message}`); process.exitCode = 1; }
}
