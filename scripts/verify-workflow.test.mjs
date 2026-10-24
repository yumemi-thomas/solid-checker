import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";

const read = path => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
const verify = read("scripts/verify.sh");

test("verification shares one build environment across every compilation before checking test targets", () => {
  const pins = verify.indexOf("step certification-pins");
  const builds = verify.indexOf("step builds-and-suites");
  assert.ok(pins > verify.indexOf("step build-typefacts"));
  assert.ok(builds > pins);
  // Every lane function is defined after the pins are exported and started by
  // the step that waits for it.
  for (const name of ["rust_lane", "typefacts_lane", "clippy_lane", "wasm_build_lane", "oracle_provision_lane"]) {
    const defined = verify.indexOf(`${name}() {`);
    assert.ok(defined > pins, name);
    assert.ok(verify.indexOf(`lane ${name.replace(/_lane$/, "").replaceAll("_", "-")} ${name}`) > builds, name);
  }
});

test("verify-performance has the machine to itself: every lane has settled before it starts", () => {
  const performance = verify.indexOf("step verify-performance");
  const lastWait = verify.lastIndexOf("lanes_wait");
  assert.ok(lastWait > verify.indexOf("step gates"));
  assert.ok(lastWait < performance);
  assert.equal(verify.slice(performance).match(/^lane /m), null);
});

test("no two lanes of one step build in one target directory", () => {
  // AGENTS.md: one Cargo process per target directory.
  const lanes = new Map(
    [...verify.matchAll(/^(\w+)_lane\(\) \{\n([\s\S]*?)^\}/gm)].map(([, name, body]) => [name, body]),
  );
  const directories = (body) => {
    const found = new Set();
    for (const command of body.replace(/\\\n/g, " ").split("\n")) {
      const explicit = /--target-dir (\S+)/.exec(command) ?? /CARGO_TARGET_DIR="([^"]+)"/.exec(command);
      if (explicit) found.add(explicit[1]);
      else if (/cargo .*apps\/solid-typefacts\/Cargo.toml|verify-tests\.mjs \S+ typefacts-test/.test(command)) found.add("apps/solid-typefacts/target");
      else if (/cargo |verify-tests\.mjs/.test(command)) found.add("rust/target");
    }
    return found;
  };
  for (const step of ["builds-and-suites", "gates"]) {
    const start = verify.indexOf(`step ${step}`);
    const started = [...verify.slice(start, verify.indexOf("lanes_wait", start)).matchAll(/^lane \S+ (\w+)_lane$/gm)];
    assert.ok(started.length > 1, step);
    const owner = new Map();
    for (const [, name] of started) {
      for (const directory of directories(lanes.get(name))) {
        assert.ok(!owner.has(directory), `${step}: ${owner.get(directory)} and ${name} both build in ${directory}`);
        owner.set(directory, name);
      }
    }
  }
});

test("the optional nextest path retains doctests and records individual timings", () => {
  const runner = read("scripts/verify-tests.mjs");
  assert.match(runner, /"nextest", "run", "--config-file", "scripts\/nextest.toml"/);
  assert.match(runner, /"test", "--profile", profile, \.\.\.common, "--doc"/);
  assert.match(read("scripts/nextest.toml"), /\[profile.verify.junit\][\s\S]*path = "junit.xml"/);
  assert.match(verify, /SOLID_CHECKER_RUST_TEST_RUNNER:-auto/);
});
