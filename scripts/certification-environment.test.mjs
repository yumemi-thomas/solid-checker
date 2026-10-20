import assert from "node:assert/strict";
import { realpathSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { test } from "vitest";
import { certificationEnvironment } from "./lib/certification-environment.mjs";

test("local Cargo drivers pin the build id and arm the test harness", () => {
  const root = fileURLToPath(new URL("../", import.meta.url));
  const node = realpathSync(process.execPath);
  const env = certificationEnvironment(root, { ...process.env, PROBE_NODE: node,
    SOLID_CHECKER_BUILD_ID: "x" });
  assert.equal(env.TYPEFACTS_BUILD_ID, "x");
  assert.equal(env.PROBE_NODE, node);
  assert.equal(env.TYPEFACTS_TEST_BIN, env.SOLID_TYPEFACTS_BIN);
});
