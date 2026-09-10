import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve } from "node:path";

const digest = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const canonical = value => JSON.stringify(value, (_key, item) =>
  item && typeof item === "object" && !Array.isArray(item)
    ? Object.fromEntries(Object.keys(item).sort().map(key => [key, item[key]])) : item);

// Inspection of this transaction's freshly certified publication, not a
// receipt verifier or a source of reusable receipt authority. The caller also
// binds the native success's selected input census; final graph publication
// independently proves every retained input again.
export function inspectRetainedCertificationFloor({ catalogRoot, expected }) {
  const root = resolve(catalogRoot);
  const read = (base, name, expectedDigest = null) => {
    if (typeof name !== "string" || isAbsolute(name)) throw new Error("retained floor has an invalid catalog reference");
    const path = resolve(base, name), rel = relative(root, path);
    if (rel.startsWith("..") || isAbsolute(rel)) throw new Error("retained floor catalog reference escapes its publication");
    const bytes = readFileSync(path), actualDigest = digest(bytes);
    if (expectedDigest !== null && actualDigest !== expectedDigest) throw new Error("retained floor publication digest mismatch");
    return { path, digest: actualDigest, value: JSON.parse(bytes) };
  };
  if (!expected.length || expected.length > 1024) throw new Error("retained floor requires a nonempty bounded accepted census");
  let catalogs, publication;
  if (existsSync(resolve(root, "accepted-contract-case-set.json"))) {
    const pointer = read(root, "accepted-contract-case-set.json");
    if (pointer.value.format !== "solid-checker-accepted-contract-case-set-pointer" || pointer.value.caseSetVersion !== 1 || typeof pointer.value.documentDigest !== "string") throw new Error("invalid retained floor case-set pointer");
    const document = read(root, pointer.value.document, pointer.value.documentDigest);
    if (document.value.format !== "solid-checker-accepted-contract-case-set" || document.value.caseSetVersion !== 1 || !Array.isArray(document.value.cases) || document.value.cases.length !== expected.length) throw new Error("retained floor case-set census mismatch");
    catalogs = document.value.cases.map(coordinate => {
      if (typeof coordinate.catalogDigest !== "string") throw new Error("retained floor catalog digest is missing");
      return { ...read(dirname(document.path), coordinate.catalog, coordinate.catalogDigest), coordinate };
    });
    publication = { pointerDigest: pointer.digest, documentDigest: document.digest };
  } else {
    catalogs = [{ ...read(root, "accepted-contracts.json"), coordinate: null }];
    publication = { catalogDigest: catalogs[0].digest };
  }
  const seen = new Set(), selected = [];
  for (const catalog of catalogs) {
    if (catalog.value.format !== "solid-checker-accepted-contract-catalog" || catalog.value.catalogVersion !== 2 || !Array.isArray(catalog.value.contracts) || catalog.value.contracts.length !== 1) throw new Error("retained floor requires one exact proposal contract per catalog");
    const entry = catalog.value.contracts[0];
    if (typeof entry.documentDigest !== "string" || typeof entry.receiptDigest !== "string") throw new Error("retained floor object digest is missing");
    const main = read(dirname(catalog.path), entry.document, entry.documentDigest).value;
    const receipt = read(dirname(catalog.path), entry.receipt, entry.receiptDigest).value;
    if (!entry.bindings || !Object.keys(entry.bindings).length || receipt.payload?.mainDigest !== entry.documentDigest ||
        Object.entries(entry.bindings).some(([key, value]) => canonical(receipt.payload?.[key]) !== canonical(value))) throw new Error("retained floor receipt binding mismatch");
    const coordinate = catalog.coordinate;
    if (coordinate && ["importer", "specifier", "resolvedImportRoot", "semanticDigest"].some(key => coordinate[key] !== entry.bindings?.[key]) || coordinate && coordinate.receiptDigest !== entry.receiptDigest) throw new Error("retained floor pointer binding mismatch");
    const imported = entry.import;
    if (!imported || imported.importer !== entry.bindings?.importer || imported.specifier !== entry.bindings?.specifier) throw new Error("retained floor importer binding mismatch");
    const entries = Object.entries(main.entrypoints ?? {});
    if (entries.length !== 1 || entries[0][1].cases?.length !== 1) throw new Error("retained floor main has an unexpected case census");
    const [entrypoint, body] = entries[0];
    const { exports, ...selection } = body.cases[0];
    if (!exports || !Object.keys(exports).length) throw new Error("retained floor main has no certified exports");
    const matches = expected.filter(item => {
      const r = item.resolution;
      return item.coordinate.entrypoint === entrypoint && imported.requestedEntrypoint === entrypoint
        && canonical(main.package) === canonical(item.package)
        && canonical(selection) === canonical(item.selection)
        && ["importer", "specifier", "packageName", "packageVersion", "packageIntegrity", "packageRoot"].every(key => imported[key] === r[key])
        && ["runtime", "declarations", "runtimeTrace", "declarationTrace"].every(key => canonical(imported[key]) === canonical(r[key]));
    });
    if (matches.length !== 1 || seen.has(matches[0])) throw new Error("retained floor does not match one exact selected input");
    seen.add(matches[0]);
    selected.push({ ...matches[0].coordinate, artifactCaseId: coordinate?.artifactCaseId ?? null,
      documentDigest: entry.documentDigest, receiptDigest: entry.receiptDigest,
      resolvedImportRoot: entry.bindings.resolvedImportRoot });
  }
  if (seen.size !== expected.length) throw new Error("retained floor omitted an accepted input");
  return { ...publication, cases: selected };
}
