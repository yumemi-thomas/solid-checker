import { test } from "bun:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { catalogSurface } from "./catalog-surface.mjs";

test("named case-set catalogs preserve open cases and reject changed bytes", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-catalog-observation-"));
  mkdirSync(join(root, "published"));
  const write = (name, value) => {
    const bytes = JSON.stringify(value);
    writeFileSync(join(root, name), bytes);
    return "sha256:" + createHash("sha256").update(bytes).digest("hex");
  };
  const document = closed => ({ package: { name: "example", version: "1.0.0" },
    summaries: { callable: { shape: "callable", call: { closed } }, constant: { shape: "plain" } },
    entrypoints: { ".": { cases: [{ exports: { helper: "callable", value: "constant" } }] } } });
  const first = write("published/first.json", document(["callbacks", "reads", "creates", "returns"]));
  const second = write("published/second.json", document(["returns"]));
  const catalog = write("published/catalog.json", { contracts: [
    { document: "first.json", documentDigest: first }, { document: "second.json", documentDigest: second }
  ] });
  const caseSet = write("published/cases.json", { cases: [{ catalog: "catalog.json", catalogDigest: catalog }] });
  write("accepted-contract-case-set.json", { caseSetVersion: 1, document: "published/cases.json", documentDigest: caseSet });
  assert.deepEqual(catalogSurface(root, "example", "1.0.0"), [
    { entrypoint: ".", export: "helper", state: "every-import" }, { entrypoint: ".", export: "value", state: "value" }
  ]);
  assert.throws(() => catalogSurface(root, "another-package", "1.0.0"), /omitted the root package/);
  writeFileSync(join(root, "published/second.json"), JSON.stringify(document(["callbacks", "reads", "creates", "returns"])));
  assert.throws(() => catalogSurface(root, "example", "1.0.0"), /Expected values to be strictly equal/);
});
