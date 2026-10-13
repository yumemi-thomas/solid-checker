// Re-run only call-time observations without repeating unchanged certification.
// Never overwrite the earlier, incorrectly guarded module-import observations.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { automaticCompositionCases, callTimeProbeSource } from "./automatic-composition-cases.mjs";

const read = path => JSON.parse(readFileSync(path));
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const run = read(resolve(process.argv[2]));
const evidenceFile = resolve(process.argv[3]), evidence = read(evidenceFile), out = resolve(process.argv[4]);
assert(!existsSync(out), "Preserve previous evidence; choose a fresh output directory");
assert.equal(evidence.kind, "automatic-composition-breadth");
assert.equal(evidence.authoredProposals, 0);
assert(process.env.SOLID_CHECKER_PROBE_NODE && existsSync(process.env.SOLID_CHECKER_PROBE_NODE));
mkdirSync(out, { recursive: true });
const document = { authority: false, kind: "automatic-composition-call-time-observations", evidenceFile,
  evidenceSha256: `sha256:${hash(readFileSync(evidenceFile))}`, guardInstalled: "after-module-import", results: [] };
for (const observed of evidence.results.filter(item => item.host === "node" && !item.refused)) {
  const spec = automaticCompositionCases.find(item => `@solid-primitives/${item.name}` === observed.package);
  const row = run.results.find(item => item.package === observed.package);
  assert.equal(row.version, spec.version); assert.equal(observed.version, spec.version);
  const root = realpathSync(join(row.retainedArtifacts.projectDir, "node_modules", row.package));
  const catalog = join(dirname(evidenceFile), `${spec.name}-node`, "accepted");
  const pointer = read(join(catalog, "accepted-contracts.json"));
  const accepted = read(join(catalog, pointer.contracts[0].document));
  assert.equal(accepted.package.name, observed.package); assert.equal(accepted.package.version, observed.version);
  assert.equal(accepted.package.integrity, observed.integrity);
  const artifact = accepted.entrypoints["."].cases[0].artifact;
  const modulePath = join(root, artifact.path);
  assert.equal(hash(readFileSync(modulePath)), artifact.sha256, "Runtime module differs from certified source");
  const source = callTimeProbeSource(spec, pathToFileURL(modulePath).href);
  writeFileSync(join(out, spec.name + ".mjs"), source + "\n");
  const process_ = Bun.spawn([process.env.SOLID_CHECKER_PROBE_NODE, "--conditions=node", "--input-type=module", "--eval", source],
    { stdout: "pipe", stderr: "pipe" });
  const [status, stdout, stderr] = await Promise.all([process_.exited, new Response(process_.stdout).text(), new Response(process_.stderr).text()]);
  document.results.push({ package: observed.package, version: observed.version, artifactSha256: `sha256:${artifact.sha256}`,
    status, stdout, stderr, observation: status === 0 ? JSON.parse(stdout) : null });
}
writeFileSync(join(out, "results.json"), JSON.stringify(document, null, 2) + "\n");
console.log(JSON.stringify(document));
