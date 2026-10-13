// Published-typing consumer twins, including the unchanged criterion-3 case.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, existsSync, mkdirSync, writeFileSync, realpathSync } from "node:fs";
import { resolve, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { oracleCompilerOptions } from "../../scripts/tsc-oracle.mjs";
const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const [runFile, evidenceFile, noneFile, outArgument] = process.argv.slice(2);
const read = path => JSON.parse(readFileSync(path)), hash = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const out = resolve(outArgument); assert(!existsSync(out)); mkdirSync(out, { recursive: true });
for (const key of ["SOLID_CHECKER_NATIVE_BIN", "SOLID_TYPEFACTS_BIN", "SOLID_CHECKER_PROBE_NODE"])
  assert(process.env[key] && existsSync(process.env[key]));
assert.equal(read(join(repo, "packages/cli/node_modules/typescript/package.json")).version, "5.9.3");
const retained = read(runFile).results.find(row => row.package === "@solid-primitives/i18n");
assert.equal(retained.version, "3.0.0-next.4");
const ledger = read(join(repo, "fixtures/primitives-misuse/cases.json")).cases.find(row => row.id === "i18n-translator-argument-read");
assert(ledger);
const targets = ["template", "identityResolveTemplate", "missingKeyAsPath"];
const specimens = targets.map(target => ({ id: target,
  misuse: `import { createSignal } from "solid-js"; import { ${target} } from "@solid-primitives/i18n";
export default function App() { const [read] = createSignal("hello"); const text = ${target}(read()); return <p>{text}</p>; }`,
  correct: `import { createSignal } from "solid-js"; import { ${target} } from "@solid-primitives/i18n";
export default function App() { const [read] = createSignal("hello"); return <p>{${target}(read())}</p>; }`
}));
specimens.push({ id: ledger.id, misuse: ledger.misuse, correct: ledger.correct });
const document = { authority: false, kind: "i18n-consumer-twins", checkerSha256: hash(readFileSync(process.env.SOLID_CHECKER_NATIVE_BIN)), results: [] };
const sourceRoot = join(realpathSync(retained.retainedArtifacts.projectDir), `solid-checker-i18n-twins-${hash(out).slice(7, 19)}`);
mkdirSync(sourceRoot, { recursive: true });
async function child(args) {
  const process_ = Bun.spawn(args, { cwd: repo, env: { ...process.env, SOLID_CHECKER_DAEMON: "0" }, stdout: "pipe", stderr: "pipe" });
  const [status, stdout, stderr] = await Promise.all([process_.exited, new Response(process_.stdout).text(), new Response(process_.stderr).text()]);
  return { status, stdout, stderr };
}
for (const specimen of specimens) {
  const item = { id: specimen.id, twins: {} }; document.results.push(item);
  for (const twin of ["misuse", "correct"]) {
    const project = join(sourceRoot, `${specimen.id}-${twin}.json`);
    writeFileSync(join(sourceRoot, `${specimen.id}-${twin}.tsx`), specimen[twin]);
    writeFileSync(project, JSON.stringify({ compilerOptions: oracleCompilerOptions("v2", true), files: [`${specimen.id}-${twin}.tsx`] }));
    const tsc = await child([process.env.SOLID_CHECKER_PROBE_NODE, join(repo, "packages/cli/node_modules/typescript/lib/tsc.js"), "--project", project]);
    assert.deepEqual(tsc, { status: 0, stdout: "", stderr: "" }, `${specimen.id}/${twin}`);
    item.twins[twin] = { tsc, hosts: {} };
    for (const host of ["none", "browser", "node"]) {
      const file = host === "none" ? noneFile : evidenceFile;
      const observed = read(file).results.find(row => row.host === host); assert(!observed.refused);
      assert.equal(observed.package, retained.package); assert.equal(observed.version, retained.version);
      assert.equal(read(file).checkerSha256, document.checkerSha256);
      const dir = join(dirname(resolve(file)), `i18n-${host}`);
      const response = await child([process.env.SOLID_CHECKER_NATIVE_BIN, "--format", "json", "--project", project,
        ...(host === "none" ? [] : ["--runtime-target", host]), "--accepted-contracts", join(dir, "accepted/accepted-contracts.json"),
        "--receipt-trust-configuration", join(dir, "trust.json")]);
      writeFileSync(join(out, `${specimen.id}-${twin}-${host}.json`), JSON.stringify(response, null, 2));
      assert([0, 1].includes(response.status), response.stderr);
      const parsed = JSON.parse(response.stdout);
      assert(parsed.packageSummaries.some(row => row.name === retained.package && row.version === retained.version && row.evidence === "accepted"));
      item.twins[twin].hosts[host] = { status: parsed.status, findings: parsed.findings.map(({ id, rule, kind }) => ({ id, rule, kind })) };
    }
  }
  writeFileSync(join(out, "results.json"), JSON.stringify(document, null, 2) + "\n");
  console.log(JSON.stringify(item));
}
