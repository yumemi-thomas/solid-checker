#!/usr/bin/env bun
// Prints one installed package's runtime export surface as JSON, under the
// Node conditions this process was started with. Spawned once per condition
// mode by scripts/generate-solid1-runtime-surface.mjs.
//
// Unlike a probe worker this does not need to run from inside the install: it
// imports the package's files by absolute path and has no bare imports of its
// own.
import { describePackages } from "./contract-probe-harness.mjs";

const [name, directory] = process.argv.slice(2);
if (!name || !directory) {
  console.error("usage: print-package-surface.mjs <package-name> <package-directory>");
  process.exit(2);
}
const packages = await describePackages({ packages: [{ name, directory }] });
process.stdout.write(JSON.stringify(packages[name]));
