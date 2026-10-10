// Replays four existing ledger twins against experimental host-specific receipts.
// No ledger expectation or accepted-tier artifact is rewritten.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { misuseVerdict } from "../../scripts/primitives-checkpoint.mjs";
import { oracleCompilerOptions } from "../../scripts/tsc-oracle.mjs";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const read = path => JSON.parse(readFileSync(path));
const hash = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const run = read(resolve(process.argv[2])), evidenceFile = resolve(process.argv[3]);
const evidence = read(evidenceFile), out = resolve(process.argv[4]);
const ledgerFile = join(repo, "fixtures/primitives-misuse/cases.json"), ledger = read(ledgerFile);
assert(!existsSync(out), "Preserve previous evidence; choose a fresh output directory");
assert.equal(evidence.authoredProposals, 0);
for (const key of ["SOLID_CHECKER_NATIVE_BIN", "SOLID_TYPEFACTS_BIN", "SOLID_CHECKER_PROBE_NODE"])
  assert(process.env[key] && existsSync(process.env[key]));
assert.equal(read(join(repo, "packages/cli/node_modules/typescript/package.json")).version, "5.9.3");
mkdirSync(out, { recursive: true });
const document = { authority: false, kind: "host-misuse-controls", ledgerSha256: hash(readFileSync(ledgerFile)),
  evidenceSha256: hash(readFileSync(evidenceFile)), checkerSha256: hash(readFileSync(process.env.SOLID_CHECKER_NATIVE_BIN)), results: [] };
const ids = ["connectivity-createConnectivitySignal-top-level-read", "connectivity-createConnectivitySignal-module-scope",
  "page-utilities-createPageVisibility-top-level-read", "page-utilities-createPageLeaveBlocker-module-scope"];
async function child(args) {
  const process_ = Bun.spawn(args, { cwd: repo, env: { ...process.env, SOLID_CHECKER_DAEMON: "0" }, stdout: "pipe", stderr: "pipe" });
  const [status, stdout, stderr] = await Promise.all([process_.exited, new Response(process_.stdout).text(), new Response(process_.stderr).text()]);
  return { status, stdout, stderr };
}
for (const id of ids) {
  const entry = ledger.cases.find(item => item.id === id); assert(entry);
  const retained = run.results.find(item => item.package === entry.package); assert.equal(retained.version, entry.version);
  const sourceRoot = realpathSync(retained.retainedArtifacts.projectDir);
  const dir = join(sourceRoot, `solid-checker-host-misuse-${hash(out).slice(7, 19)}`, id); mkdirSync(dir, { recursive: true });
  const item = { id, package: entry.package, export: entry.export, expectedRule: entry.rule, expectedKind: entry.kind ?? "violation", hosts: {} };
  document.results.push(item);
  const tsc = {};
  for (const twin of ["misuse", "correct"]) {
    writeFileSync(join(dir, `${twin}.tsx`), entry[twin]);
    writeFileSync(join(dir, `${twin}.json`), JSON.stringify({ compilerOptions: oracleCompilerOptions("v2", true), files: [`${twin}.tsx`] }));
    const response = await child([process.env.SOLID_CHECKER_PROBE_NODE, join(repo, "packages/cli/node_modules/typescript/lib/tsc.js"), "--project", join(dir, `${twin}.json`)]);
    assert.deepEqual(response, { status: 0, stdout: "", stderr: "" }, `${id}/${twin}: published typings must be silent`);
    tsc[twin] = [];
  }
  item.tsc = tsc;
  for (const host of ["node", "browser"]) {
    assert(entry.hosts.includes(host));
    const observed = evidence.results.find(row => row.package === entry.package && row.host === host);
    assert(!observed.refused); assert.equal(observed.version, entry.version);
    const name = entry.package.slice("@solid-primitives/".length);
    const catalog = join(dirname(evidenceFile), `${name}-${host}`, "accepted/accepted-contracts.json");
    const trust = join(dirname(evidenceFile), `${name}-${host}`, "trust.json");
    const findings = {};
    item.hosts[host] = { findings };
    for (const twin of ["misuse", "correct"]) {
      const response = await child([process.env.SOLID_CHECKER_NATIVE_BIN, "--format", "json", "--project", join(dir, `${twin}.json`),
        "--runtime-target", host, "--accepted-contracts", catalog, "--receipt-trust-configuration", trust]);
      writeFileSync(join(out, `${id}-${host}-${twin}.json`), JSON.stringify(response, null, 2) + "\n");
      assert([0, 1].includes(response.status), response.stderr);
      const parsed = JSON.parse(response.stdout);
      assert(parsed.packageSummaries.some(row => row.name === entry.package && row.version === entry.version && row.evidence === "accepted"));
      findings[twin] = parsed.findings.map(({ id, rule, kind }) => ({ id, rule, kind }));
      if (host === "node") {
        assert.equal(parsed.status, "certified", `${id}/${twin}`);
        assert.deepEqual(findings[twin], [], `${id}/${twin}`);
      }
    }
    item.hosts[host].ledgerVerdict = misuseVerdict({ rule: entry.rule, kind: entry.kind, tsc,
      misuseFindings: findings.misuse, correctFindings: findings.correct });
    if (host === "node") assert.equal(item.hosts[host].ledgerVerdict.status, "misuse silent");
  }
  writeFileSync(join(out, "results.json"), JSON.stringify(document, null, 2) + "\n");
}
assert.equal(hash(readFileSync(ledgerFile)), document.ledgerSha256, "Ledger was changed");
console.log(JSON.stringify({ strictTypeChecks: ids.length * 2, consumerAnalyses: ids.length * 4,
  certifiedNodeTwins: ids.length * 2, contradictoryNodeExpectations: ids.length }));
