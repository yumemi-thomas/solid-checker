import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";

const read = path => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
const verify = read("scripts/verify.sh");

test("verification pins every compilation before checking test targets", () => {
  const pins = verify.indexOf("export SOLID_TYPEFACTS_CERTIFICATION_SHA256");
  const preflight = verify.indexOf("step clippy");
  const tests = verify.indexOf("step go-rust-tests");
  assert.ok(pins > verify.indexOf("step build-typefacts"));
  assert.ok(preflight > pins);
  assert.ok(tests > preflight);
  assert.ok(tests > verify.indexOf("step oracle-provision"));
  assert.ok(verify.indexOf('wait "$tests_pid" || tests_status=$?') < verify.indexOf("step verify-performance"));
});

test("the optional nextest path retains doctests and records individual timings", () => {
  const runner = read("scripts/verify-tests.mjs");
  assert.match(runner, /"nextest", "run", "--config-file", "scripts\/nextest.toml"/);
  assert.match(runner, /"test", "--profile", profile, \.\.\.common, "--doc"/);
  assert.match(read("scripts/nextest.toml"), /\[profile.verify.junit\][\s\S]*path = "junit.xml"/);
});

test("both conformance Cargo drivers honor the verification profile", () => {
  assert.match(verify, /SOLID_CHECKER_CARGO_PROFILE="\$cargo_profile"/);
  for (const path of ["scripts/check-bundled-contracts.mjs", "scripts/dialect-manifests.mjs"]) {
    assert.match(read(path), /"--profile",\s*process.env.SOLID_CHECKER_CARGO_PROFILE \|\| "dev"/);
  }
});
