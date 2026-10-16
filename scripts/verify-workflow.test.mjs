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
  // The citation test's archive-reading arm reads these roots.
  const archives = verify.indexOf("step archives-provision");
  assert.ok(archives > -1 && tests > archives);
  assert.ok(verify.indexOf("export SOLID_CHECKER_RC3_ARCHIVE_ROOT SOLID_CHECKER_RC6_ARCHIVE_ROOT SOLID_CHECKER_RC9_ARCHIVE_ROOT SOLID_CHECKER_RC13_ARCHIVE_ROOT") > archives);
  assert.ok(verify.indexOf('wait "$tests_pid" || tests_status=$?') < verify.indexOf("step verify-performance"));
});

test("the optional nextest path retains doctests and records individual timings", () => {
  const runner = read("scripts/verify-tests.mjs");
  assert.match(runner, /"nextest", "run", "--config-file", "scripts\/nextest.toml"/);
  assert.match(runner, /"test", "--profile", profile, \.\.\.common, "--doc"/);
  assert.match(read("scripts/nextest.toml"), /\[profile.verify.junit\][\s\S]*path = "junit.xml"/);
});
