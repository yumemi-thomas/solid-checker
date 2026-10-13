// Read only the native publisher's named plain/case-set catalogs. This is an
// observation reader, not a receipt verifier or alternate admission policy.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { consumerState } from "../../scripts/contract-coverage-census.mjs";

export const surfaceClosed = state => state === "clean" || state === "value";
function referenced(base, reference, digest) {
  assert.equal(typeof reference, "string");
  const path = resolve(base, reference), offset = relative(resolve(base), path);
  assert(offset && !offset.startsWith("..") && !offset.startsWith("/"), "Catalog reference escapes its directory");
  const bytes = readFileSync(path);
  assert.equal("sha256:" + createHash("sha256").update(bytes).digest("hex"), digest);
  return { path, document: JSON.parse(bytes) };
}
export function catalogSurface(catalog, packageName, version) {
  const catalogs = [], plain = join(catalog, "accepted-contracts.json");
  if (existsSync(plain)) catalogs.push(plain);
  const pointerPath = join(catalog, "accepted-contract-case-set.json");
  if (existsSync(pointerPath)) {
    const pointer = JSON.parse(readFileSync(pointerPath));
    const references = pointer.caseSetVersion === 1 ? [pointer]
      : pointer.caseSetVersion === 2 ? pointer.caseSets : null;
    assert(Array.isArray(references), "Unknown case-set pointer version");
    for (const reference of references) {
      const { path, document } = referenced(catalog, reference.document, reference.documentDigest);
      assert(Array.isArray(document.cases));
      for (const entry of document.cases)
        catalogs.push(referenced(dirname(path), entry.catalog, entry.catalogDigest).path);
    }
  }
  assert(catalogs.length > 0, "Native transaction issued no accepted root catalog");
  const surfaces = new Map();
  for (const path of new Set(catalogs)) {
    const pointer = JSON.parse(readFileSync(path)); assert(Array.isArray(pointer.contracts));
    for (const reference of pointer.contracts) {
      const { document } = referenced(dirname(path), reference.document, reference.documentDigest);
      if (document.package.name !== packageName || document.package.version !== version) continue;
      for (const [entrypoint, value] of Object.entries(document.entrypoints))
        for (const artifact of value.cases)
          for (const [name, summary] of Object.entries(artifact.exports)) {
            const identity = JSON.stringify([entrypoint, name]), previous = surfaces.get(identity);
            const state = consumerState(document, summary);
            surfaces.set(identity, { entrypoint, export: name,
              state: previous && !surfaceClosed(previous.state) ? previous.state : state });
          }
    }
  }
  assert(surfaces.size > 0, "Published catalogs omitted the root package");
  return [...surfaces.values()];
}
