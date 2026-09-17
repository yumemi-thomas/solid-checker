// A package investigation owns its build and output directory. The raw runner
// remains usable by CI, where the build is scheduled separately.
import { spawnSync } from "node:child_process";
import { readFileSync, mkdirSync, mkdtempSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));

export function packageInvestigation({ PACKAGE, ECOSYSTEM_PROFILE = "release" }, manifest) {
  if (!PACKAGE || !manifest.rows.some(row => row.package === PACKAGE && row.probes?.length)) {
    throw new Error("PACKAGE must name an exact official package with probes in the ecosystem manifest");
  }
  if (!["debug", "release"].includes(ECOSYSTEM_PROFILE)) throw new Error("ECOSYSTEM_PROFILE must be debug or release");
  return {
    target: `build-checker-${ECOSYSTEM_PROFILE}`,
    binary: join(root, "rust/target", ECOSYSTEM_PROFILE, "solid-checker-rust"),
    args: ["scripts/ecosystem-benchmark/run.mjs", "--package", PACKAGE,
      "--attempt-certification", "--probe-recipe-corpus", "scripts/ecosystem-benchmark/probe-recipes",
      "--include-graph", "--keep-temp", "--timeout", "600"]
  };
}

export function main(env = process.env) {
  const manifest = JSON.parse(readFileSync(join(root, "scripts/ecosystem-benchmark/manifest.json"), "utf8"));
  const plan = packageInvestigation(env, manifest);
  const built = spawnSync("make", [plan.target], { cwd: root, env, stdio: "inherit" });
  if (built.error || built.status !== 0) return built.status || 1;
  const parent = join(root, "rust/target/ecosystem-investigations");
  mkdirSync(parent, { recursive: true });
  const output = mkdtempSync(join(parent, "package-"));
  console.log(`Package investigation: ${output}\nTemporary probe projects and their audit sidecars will also be retained.`);
  const result = spawnSync(process.execPath, [...plan.args,
    "--json", join(output, "report.json"), "--markdown", join(output, "report.md")], {
    cwd: root, stdio: "inherit", env: { ...env,
      SOLID_CHECKER_NATIVE_BIN: plan.binary,
      SOLID_TYPEFACTS_BIN: join(root, "bin/solid-typefacts") }
  });
  return result.status ?? 1;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { process.exitCode = main(); }
  catch (error) { console.error(`ecosystem-package: ${error.message}`); process.exitCode = 1; }
}
