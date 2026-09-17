import assert from "node:assert/strict";
import { readFileSync, realpathSync } from "node:fs";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
import { test } from "vitest";
import { certificationEnvironment } from "./lib/certification-environment.mjs";
import { sourceDigest } from "./typefacts-source-identity.mjs";

test("local Cargo drivers replace stale inherited pins and arm the test harness", () => {
  const root = fileURLToPath(new URL("../", import.meta.url));
  const node = realpathSync(process.execPath);
  const env = certificationEnvironment(root, { ...process.env, PROBE_NODE: node,
    PROBE_BROWSER: "", SOLID_TYPEFACTS_CERTIFICATION_SHA256: "stale",
    SOLID_CHECKER_PROBE_NODE_SHA256: "stale", SOLID_CHECKER_PROBE_BROWSER_SHA256: "stale" });
  const sha = path => `sha256:${createHash("sha256").update(readFileSync(path)).digest("hex")}`;
  assert.equal(env.SOLID_TYPEFACTS_CERTIFICATION_SHA256, sha(`${root}/bin/solid-typefacts`));
  assert.equal(env.SOLID_TYPEFACTS_SOURCE_MANIFEST_SHA256, `sha256:${sourceDigest(root)}`);
  assert.equal(env.SOLID_CHECKER_PROBE_NODE_SHA256, sha(node));
  assert.equal(env.SOLID_CHECKER_EXPECT_PROBE_PINS, "1");
  assert.equal(env.PROBE_NODE, node);
  assert.equal(env.SOLID_CHECKER_PROBE_BROWSER_SHA256, undefined);
  assert.equal(env.TYPEFACTS_TEST_BIN, env.SOLID_TYPEFACTS_BIN);
});
