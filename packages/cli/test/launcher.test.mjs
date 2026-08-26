import assert from "node:assert/strict";
import { chmodSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { test } from "vitest";

test("forwards arguments, output, environment, and the native exit code", () => {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-cli-"));
  const native = join(directory, "solid-checker-native");
  const capture = join(directory, "arguments");
  writeFileSync(native, `#!/bin/sh
printf '%s\n' "$@" > "$SOLID_CHECKER_TEST_CAPTURE"
printf 'native stdout\n'
printf 'native stderr\n' >&2
exit 7
`);
  chmodSync(native, 0o700);

  const result = spawnSync(process.execPath, [
    new URL("../bin/solid-checker.mjs", import.meta.url).pathname,
    "--project",
    "tsconfig.json",
    "--format",
    "json"
  ], {
    encoding: "utf8",
    env: {
      ...process.env,
      SOLID_CHECKER_NATIVE_BIN: native,
      SOLID_CHECKER_TEST_CAPTURE: capture
    }
  });

  assert.equal(result.status, 7);
  assert.equal(result.stdout, "native stdout\n");
  assert.equal(result.stderr, "native stderr\n");
  assert.deepEqual(readFileSync(capture, "utf8").trim().split("\n"), [
    "--project",
    "tsconfig.json",
    "--format",
    "json"
  ]);
});

test("loads the current platform's optional native package", () => {
  const packageRoot = new URL("..", import.meta.url).pathname;
  const suffix = {
    "darwin-arm64": "darwin-arm64",
    "linux-arm64": "linux-arm64-gnu",
    "linux-x64": "linux-x64-gnu",
    "win32-x64": "win32-x64-msvc"
  }[`${process.platform}-${process.arch}`];
  // A platform with no published binding is a supported way to run from a
  // checkout, so this asserts nothing there rather than failing.
  if (!suffix) return;
  const dependency = `@solid-checker/binding-${suffix}`;
  const dependencyRoot = join(packageRoot, "node_modules", dependency);
  const nativeRoot = join(dependencyRoot, "native", `${process.platform}-${process.arch}`);
  const native = join(nativeRoot, `solid-checker${process.platform === "win32" ? ".exe" : ""}`);
  const packageJson = join(dependencyRoot, "package.json");

  mkdirSync(nativeRoot, { recursive: true });
  writeFileSync(packageJson, JSON.stringify({ name: dependency, version: "0.0.0" }));
  writeFileSync(native, process.platform === "win32"
    ? "@echo off\r\nexit /b 23\r\n"
    : "#!/bin/sh\nexit 23\n");
  chmodSync(native, 0o700);

  try {
    const result = spawnSync(process.execPath, [
      new URL("../bin/solid-checker.mjs", import.meta.url).pathname
    ], {
      env: {
        ...process.env,
        SOLID_CHECKER_NATIVE_BIN: ""
      }
    });
    assert.equal(result.status, 23);
  } finally {
    rmSync(dependencyRoot, { recursive: true, force: true });
  }
});

test("contract check forwards --check-contracts and the native exit code", () => {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-cli-"));
  const native = join(directory, "solid-checker-native");
  const capture = join(directory, "arguments");
  writeFileSync(native, `#!/bin/sh
printf '%s\n' "$@" > "$SOLID_CHECKER_TEST_CAPTURE"
printf 'reactive-package: stale\n'
exit 1
`);
  chmodSync(native, 0o700);

  const result = spawnSync(process.execPath, [
    new URL("../bin/solid-checker.mjs", import.meta.url).pathname,
    "contract",
    "check",
    "--project",
    "tsconfig.json"
  ], {
    encoding: "utf8",
    env: {
      ...process.env,
      SOLID_CHECKER_NATIVE_BIN: native,
      SOLID_CHECKER_TEST_CAPTURE: capture
    }
  });

  // A package needing action must reach CI as a non-zero exit, not just as text.
  assert.equal(result.status, 1);
  assert.equal(result.stdout, "reactive-package: stale\n");
  assert.deepEqual(readFileSync(capture, "utf8").trim().split("\n"), [
    "--check-contracts",
    "--project",
    "tsconfig.json"
  ]);
});
