// Offline measurement runner. Uses cached published archives and the real CLI
// certification path; any registry cache miss fails instead of using the network.
import { randomBytes } from "node:crypto";
import { mkdtempSync, realpathSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { certifyContract } from "../../../packages/cli/scripts/certify-contract.mjs";

const { values } = parseArgs({
  options: {
    "package-root": { type: "string" },
    integrity: { type: "string" },
    conditions: { type: "string" },
    entrypoint: { type: "string", default: "." },
    "probe-recipe-corpus": { type: "string" },
    "dependency-graph-lane": { type: "boolean", default: false },
    "registry-cache": { type: "string" },
    "output-parent": { type: "string", default: tmpdir() },
  },
});
if (!values["package-root"] || !values.integrity) {
  throw new Error("Required: --package-root <installed package> --integrity <exact SRI>");
}
if (!process.env.SOLID_CHECKER_NATIVE_BIN || !process.env.SOLID_TYPEFACTS_BIN) {
  throw new Error("Set SOLID_CHECKER_NATIVE_BIN and SOLID_TYPEFACTS_BIN to the pinned binaries.");
}
const repository = fileURLToPath(new URL("../../../", import.meta.url));
const packageRoot = realpathSync(values["package-root"]);
const output = mkdtempSync(join(values["output-parent"], "returns-synthesis-offline-"));
process.env.SOLID_CHECKER_REGISTRY_CACHE =
  values["registry-cache"] || process.env.SOLID_CHECKER_REGISTRY_CACHE || join(repository, "rust/target/registry-cache");
process.env.SOLID_CHECKER_PROBE_NODE = realpathSync(process.execPath);
writeFileSync(join(output, "issuer.json"), JSON.stringify({
  format: "solid-checker-policy2-issuer-configuration",
  issuerConfigurationVersion: 1,
  kind: "persistent-local",
  scope: "returns-synthesis-offline-measurement",
  seed: randomBytes(32).toString("base64"),
  revocationEpoch: 1,
}), { mode: 0o600 });
console.log(JSON.stringify({ output, packageRoot, nativeBin: process.env.SOLID_CHECKER_NATIVE_BIN }));
try {
  const result = await certifyContract([
    "--package-root", packageRoot,
    "--integrity", values.integrity,
    "--registry-origin", "https://registry.npmjs.org",
    "--entrypoint", values.entrypoint,
    ...(values["dependency-graph-lane"] ? ["--dependency-graph-lane"] : []),
    ...(values.conditions ? ["--conditions", values.conditions] : []),
    "--issuer-configuration", join(output, "issuer.json"),
    "--trust-configuration-output", join(output, "trust.json"),
    "--catalog", join(output, "accepted-contracts.json"),
    "--probe-recipe-corpus", values["probe-recipe-corpus"] || join(repository, "scripts/ecosystem-benchmark/probe-recipes"),
    "--audit-output", join(output, "audit.json"),
  ], {
    fetch_: async url => { throw new Error(`offline certification cache miss: ${url}`); },
  });
  console.log(JSON.stringify({ output, result }));
} catch (error) {
  console.error(JSON.stringify({ output, error: error.message }));
  process.exitCode = 1;
}
