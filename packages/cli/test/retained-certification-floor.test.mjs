import { test } from "vitest";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { inspectRetainedCertificationFloor } from "../scripts/retained-certification-floor.mjs";

const hash = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
function publication(root, entrypoints = [".", "./other"], mutate = () => {}) {
  const write = (path, value) => { const bytes = JSON.stringify(value); mkdirSync(dirname(path), { recursive: true }); writeFileSync(path, bytes); return hash(bytes); };
  const expected = [], cases = [];
  for (const [index, entrypoint] of entrypoints.entries()) {
    const coordinate = { entrypoint, conditions: ["import"] };
    const package_ = { name: "pkg", version: "1.0.0", integrity: "pin" };
    const selection = { artifact: { path: `./${index}.js`, sha256: `runtime-${index}`, closureSha256: `closure-${index}` }, declarations: { path: `./${index}.d.ts`, sha256: `types-${index}` }, resolution: { runtimeBranch: "/import", typesBranch: "/types" } };
    const resolution = { importer: "/project/app.mjs", specifier: index ? "pkg/other" : "pkg", requestedEntrypoint: entrypoint,
      packageRoot: "/pkg", packageName: "pkg", packageVersion: "1.0.0", packageIntegrity: "pin",
      runtime: { path: `/pkg/${index}.js`, digest: `sha256:runtime-${index}` }, declarations: { path: `/pkg/${index}.d.ts`, digest: `sha256:types-${index}` }, runtimeTrace: { branch: "/import" }, declarationTrace: { branch: "/types" } };
    expected.push({ coordinate, package: package_, selection, resolution });
    const main = { package: package_, entrypoints: { [entrypoint]: { cases: [{ ...selection, exports: { value: "summary" } }] } }, summaries: { summary: { shape: "plain" } } };
    const bindings = { importer: resolution.importer, specifier: resolution.specifier, resolvedImportRoot: `root-${index}`, semanticDigest: `semantic-${index}` };
    const entry = { document: "main.json", receipt: "receipt.json", import: structuredClone(resolution), bindings: structuredClone(bindings) };
    mutate({ main, entry, index });
    const base = join(root, "case-sets", "set", `case-${index}`);
    entry.documentDigest = write(join(base, "main.json"), main);
    entry.receiptDigest = write(join(base, "receipt.json"), { payload: { ...entry.bindings, mainDigest: entry.documentDigest } });
    const catalog = { format: "solid-checker-accepted-contract-catalog", catalogVersion: 2, contracts: [entry] };
    const catalogDigest = write(join(base, "accepted-contracts.json"), catalog);
    cases.push({ ...bindings, artifactCaseId: `case-${index}`, receiptDigest: entry.receiptDigest, catalog: `case-${index}/accepted-contracts.json`, catalogDigest });
  }
  const document = "case-sets/set/accepted-contract-case-set.json";
  const documentDigest = write(join(root, document), { format: "solid-checker-accepted-contract-case-set", caseSetVersion: 1, cases });
  write(join(root, "accepted-contract-case-set.json"), { format: "solid-checker-accepted-contract-case-set-pointer", caseSetVersion: 1, document, documentDigest });
  return expected;
}

test("retained floor follows the named catalog census and binds exact artifact and importer selections", () => {
  const root = mkdtempSync(join(tmpdir(), "retained-floor-"));
  try {
    const expected = publication(root);
    writeFileSync(join(root, "accepted-contracts.json"), "not the published index");
    const got = inspectRetainedCertificationFloor({ catalogRoot: root, expected });
    assert.deepEqual(got.cases.map(c => c.entrypoint), [".", "./other"]);
    assert.ok(got.cases.every(c => c.documentDigest.startsWith("sha256:") && c.receiptDigest.startsWith("sha256:")));
    assert.throws(() => inspectRetainedCertificationFloor({ catalogRoot: root, expected: expected.slice(1) }), /census mismatch/);
    assert.throws(() => inspectRetainedCertificationFloor({ catalogRoot: root, expected: [expected[0], expected[0]] }), /one exact selected input/);
    writeFileSync(join(root, "case-sets/set/case-0/main.json"), "{}");
    assert.throws(() => inspectRetainedCertificationFloor({ catalogRoot: root, expected }), /digest mismatch/);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test("retained floor rejects coherently hashed publications for different artifacts or contexts", () => {
  for (const mutate of [
    ({ main }) => { main.package.version = "2.0.0"; },
    ({ main }) => { Object.values(main.entrypoints)[0].cases[0].artifact.sha256 = "other"; },
    ({ main }) => { Object.values(main.entrypoints)[0].cases[0].artifact.closureSha256 = "other"; },
    ({ main }) => { Object.values(main.entrypoints)[0].cases[0].resolution.typesBranch = "/other"; },
    ({ entry }) => { entry.import.importer = "/other.mjs"; },
    ({ entry }) => { entry.import.declarations.digest = "sha256:other"; },
    ({ entry }) => { entry.bindings.resolvedImportRoot = "other"; }
  ]) {
    const root = mkdtempSync(join(tmpdir(), "retained-floor-negative-"));
    try {
      // Keep expected source inputs independent of mutations to the publication.
      const expected = structuredClone(publication(root));
      publication(root, [".", "./other"], mutate);
      assert.throws(() => inspectRetainedCertificationFloor({ catalogRoot: root, expected }), /retained floor/);
    } finally { rmSync(root, { recursive: true, force: true }); }
  }
});

test("malformed pointers do not fall back to a leftover catalog", () => {
  const root = mkdtempSync(join(tmpdir(), "retained-floor-pointer-"));
  try {
    const expected = publication(root);
    const path = join(root, "accepted-contract-case-set.json"), original = JSON.parse(readFileSync(path));
    for (const pointer of [{ ...original, format: "other" }, { ...original, document: "../outside.json" }, { ...original, documentDigest: "sha256:wrong" }]) {
      writeFileSync(path, JSON.stringify(pointer));
      assert.throws(() => inspectRetainedCertificationFloor({ catalogRoot: root, expected }), /retained floor/);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
});
