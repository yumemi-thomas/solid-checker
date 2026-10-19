// Environment shared by local Cargo drivers: build id, probe Node, and the
// armed Type Facts test binary.
import { execFileSync } from "node:child_process";
import { realpathSync } from "node:fs";
import { join } from "node:path";

export function certificationEnvironment(root, environment = process.env) {
  const node = realpathSync(environment.PROBE_NODE || execFileSync("node", [
    "-e", "process.stdout.write(require('fs').realpathSync(process.execPath))"
  ], { env: environment, encoding: "utf8" }).trim());
  const buildId = environment.SOLID_CHECKER_BUILD_ID || "dev";
  const result = {
    ...environment,
    PROBE_NODE: node,
    SOLID_CHECKER_BUILD_ID: buildId,
    TYPEFACTS_BUILD_ID: buildId,
    TYPEFACTS_TEST_BIN: join(root, "bin/solid-typefacts"),
    SOLID_TYPEFACTS_BIN: join(root, "bin/solid-typefacts"),
  };
  return result;
}
