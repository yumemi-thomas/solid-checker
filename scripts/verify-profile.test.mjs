import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";
import { CHECKS } from "./verify-delta.mjs";
import { verificationJobs } from "./verify-tests.mjs";

const cargoManifest = readFileSync("rust/Cargo.toml", "utf8");
const verifyScript = readFileSync("scripts/verify.sh", "utf8");

test("full verification builds in the edit loop's dev profile, so a handoff reuses its build", () => {
  assert.doesNotMatch(cargoManifest, /\[profile\.verify\]/);
  assert.match(verifyScript, /cargo_profile=dev/);
  for (const runner of ["test", "nextest"]) {
    for (const command of verificationJobs(runner, {})[1].commands) {
      const option = command.includes("nextest") ? "--cargo-profile" : "--profile";
      assert.equal(command[command.indexOf(option) + 1], "dev");
    }
  }
  assert.match(verifyScript, /--profile "\$cargo_profile"/);
  // Cargo writes the `dev` profile under `debug/`.
  assert.match(verifyScript, /checker_bin="\$PWD\/rust\/target\/debug\/solid-checker-rust"/);
});

test("clippy has a target directory of its own, shared with verify-delta", () => {
  assert.match(verifyScript, /clippy --profile "\$cargo_profile" --target-dir rust\/target\/lint/);
  assert.deepEqual(CHECKS.clippy.command.slice(2, 5), ["clippy", "--target-dir", "rust/target/lint"]);
});
