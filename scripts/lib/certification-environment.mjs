// Compile-time inputs shared by local Cargo drivers. Compute only after the
// producer has been brought current; never inherit stale certification pins.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, realpathSync } from "node:fs";
import { join } from "node:path";
import { sourceDigest as typefactsDigest } from "../typefacts-source-identity.mjs";

export function certificationEnvironment(root, environment = process.env) {
  const digest = path => `sha256:${createHash("sha256").update(readFileSync(path)).digest("hex")}`;
  const node = realpathSync(environment.PROBE_NODE || execFileSync("node", [
    "-e", "process.stdout.write(require('fs').realpathSync(process.execPath))"
  ], { env: environment, encoding: "utf8" }).trim());
  const buildId = environment.SOLID_CHECKER_BUILD_ID || "dev";
  const result = {
    ...environment,
    PROBE_NODE: node,
    SOLID_CHECKER_BUILD_ID: buildId,
    TYPEFACTS_BUILD_ID: buildId,
    SOLID_TYPEFACTS_CERTIFICATION_SHA256: digest(join(root, "bin/solid-typefacts")),
    SOLID_TYPEFACTS_SOURCE_MANIFEST_SHA256: `sha256:${typefactsDigest(root)}`,
    SOLID_CHECKER_PROBE_HARNESS_SHA256: `sha256:${execFileSync(node, [
      join(root, "scripts/probe-harness-source-identity.mjs"), "--build-id", buildId, "--write-stamp", "--digest"
    ], { env: environment, encoding: "utf8" }).trim()}`,
    SOLID_CHECKER_PROBE_NODE_SHA256: digest(node),
    SOLID_CHECKER_EXPECT_PROBE_PINS: "1",
    TYPEFACTS_TEST_BIN: join(root, "bin/solid-typefacts"),
    SOLID_TYPEFACTS_BIN: join(root, "bin/solid-typefacts"),
    SOLID_CHECKER_RC3_ARCHIVE_ROOT: join(root, "rust/target/tsc-oracle/v2/node_modules")
  };
  delete result.SOLID_CHECKER_PROBE_BROWSER_SHA256;
  delete result.SOLID_CHECKER_EXPECT_BROWSER_PIN;
  if (environment.PROBE_BROWSER) {
    result.SOLID_CHECKER_PROBE_BROWSER_SHA256 = `sha256:${execFileSync(node, [
      join(root, "scripts/probe-browser-identity.mjs"), environment.PROBE_BROWSER
    ], { env: environment, encoding: "utf8" }).trim()}`;
    result.SOLID_CHECKER_EXPECT_BROWSER_PIN = "1";
  }
  return result;
}
