import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";

const read = path => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
const verify = read("scripts/verify.sh");

test("verification shares one build environment across every compilation before checking test targets", () => {
  const pins = verify.indexOf("step certification-pins");
  const preflight = verify.indexOf("step clippy");
  const tests = verify.indexOf("step rust-tests");
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
