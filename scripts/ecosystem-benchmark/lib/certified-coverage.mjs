// How much of a package a *certified* attempt actually covered.
//
// "Verified" was one word for two very different outcomes. A row whose four
// declared entrypoints all certify and a row where one of four certified --
// and not the root -- both read as verified, and the corpus-wide rate could
// therefore rise while the surface under receipt shrank. The split is
// measurable, so it is measured.
//
// The authority is the published catalog, not the proposal: what a proposal
// claimed is not what a receipt covers, and the two lanes disagree on purpose
// (the published-dependency-graph lane certifies exactly the artifact cases the
// plain lane refused, and a reused proposal exactly the ones it generated).
// Nothing here reinterprets an absent contract as a negative claim: a catalog
// that cannot be read -- or that holds no document for the row's exact package
// identity, or whose row has no readable denominator -- is `null`, which is
// "not measured", never "covered nothing".

import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";

const CATALOG_FILE = "accepted-contracts.json";

const CASE_SET_POINTER_FILE = "accepted-contract-case-set.json";

/// Every `accepted-contracts.json` this publication names at `catalogPath`.
///
/// Two layouts exist and each is read on its own terms:
///
///   * a single-case publication writes one catalog at the catalog root;
///   * a case-set publication writes `accepted-contract-case-set.json` at the
///     root, pointing at a case-set document under `case-sets/<digest>/`, whose
///     every case names its own `cases/<resolved-import-root>/` catalog
///     *relative to that document* (`policy2_case_catalog_path` in
///     `solid-facts-backend`).
///
/// The case-set layout is walked by following the pointer, not by globbing for
/// `cases/` directories: the pointer is the published index, and a glob would
/// count a catalog the case set does not name -- a leftover from an earlier
/// publication to the same root would then inflate coverage.
///
/// The two layouts are read *exclusively*, for exactly the same reason. A
/// parsed pointer says this root is a case-set publication, so its index names
/// every catalog the publication has; a root `accepted-contracts.json` sitting
/// beside it is a leftover single-case catalog from an earlier publication to
/// the same root, and reading it would credit the row with entrypoints this
/// receipt does not cover. A pointer that parses but whose case-set document
/// does not is answered with no catalogs at all -- "not measured" -- rather
/// than by falling back to the root catalog the pointer supersedes.
function catalogFiles(catalogPath) {
  const pointer = readJson(join(catalogPath, CASE_SET_POINTER_FILE));
  if (
    pointer?.format === "solid-checker-accepted-contract-case-set-pointer" &&
    typeof pointer.document === "string"
  ) {
    const documentPath = join(catalogPath, pointer.document);
    const caseSet = readJson(documentPath);
    if (
      caseSet?.format !== "solid-checker-accepted-contract-case-set" ||
      !Array.isArray(caseSet.cases)
    ) {
      return [];
    }
    const base = dirname(documentPath);
    const files = [];
    for (const entry of caseSet.cases) {
      if (typeof entry?.catalog !== "string") continue;
      const file = join(base, entry.catalog);
      if (existsSync(file) && !files.includes(file)) files.push(file);
    }
    return files;
  }
  const root = join(catalogPath, CATALOG_FILE);
  return existsSync(root) ? [root] : [];
}

function readJson(path) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    return null;
  }
}

/// The entrypoints of `packageName@packageVersion` that the catalog at
/// `catalogPath` carries an accepted contract document for.
///
/// Filtered by exact package *identity*, name and version both: the graph lane
/// publishes the root package's dependencies into the same catalog, and
/// counting those as the row's own coverage would report a row as more covered
/// the more dependencies it needed. The version is not redundant with the name
/// -- a graph node can be another version of the row's own package (a
/// `@solidjs/signals` row whose graph pulls a different `@solidjs/signals`
/// prerelease is exactly the corpus shape), and crediting its entrypoints to
/// this row, or letting its `.` set `rootCertified`, would report a receipt
/// nobody issued for these bytes.
///
/// An absent version is answered `null` -- not measured -- rather than by
/// matching on the name alone: identity is the whole filter, and a caller that
/// cannot name the version cannot be told which documents are the row's.
export function certifiedEntrypointsOf({ catalogPath, packageName, packageVersion }) {
  if (typeof catalogPath !== "string" || !catalogPath || !existsSync(catalogPath)) return null;
  if (typeof packageVersion !== "string" || !packageVersion) return null;
  const entrypoints = new Set();
  let read = false;
  for (const file of catalogFiles(catalogPath)) {
    const catalog = readJson(file);
    if (catalog?.format !== "solid-checker-accepted-contract-catalog") continue;
    if (!Array.isArray(catalog.contracts)) continue;
    read = true;
    for (const entry of catalog.contracts) {
      if (typeof entry?.document !== "string") continue;
      // Relative to the catalog *file's* own directory, which is how the
      // consumer resolves it (`contract_interface.rs`'s `base`): a case-set
      // catalog under `cases/<key>/` names its objects beside itself, not
      // beside the catalog root.
      const document = readJson(join(dirname(file), entry.document));
      if (
        document?.format !== "solid-reactivity-contract" ||
        document?.package?.name !== packageName ||
        document?.package?.version !== packageVersion ||
        !document.entrypoints ||
        typeof document.entrypoints !== "object"
      ) {
        continue;
      }
      for (const entrypoint of Object.keys(document.entrypoints)) entrypoints.add(entrypoint);
    }
  }
  // A catalog that parses but carries no document for this exact identity is
  // not a measurement of zero coverage. A certified row published a receipt by
  // construction, so a catalog with nothing for it means this reader could not
  // find what that receipt covered -- unmeasured, never "covered nothing",
  // which the summary would count as a partial row.
  return read && entrypoints.size > 0 ? [...entrypoints].sort() : null;
}

/// `{ declaredEntrypoints, declaredWildcard, certifiedEntrypoints,
/// rootCertified }`, or `null` when no catalog was published (a refused or
/// failed attempt), none could be parsed, or none carries a document for this
/// exact package identity.
///
/// `declaredEntrypoints` is the row's own already-measured count of what
/// `package.json` declares, carried here so a reader does not have to join two
/// places to see `k of n`. It may be `null` for a package whose manifest could
/// not be read, and then it stays `null` -- an unknown denominator is not zero.
///
/// `declaredWildcard` travels with it because the count alone cannot say
/// whether it is a denominator: it is set by the manifest reader when any
/// `exports` key contains `*`, and only that reader can see it. Comparing the
/// two numbers is not a substitute -- a wildcard manifest whose expansion
/// happens to certify exactly as many entrypoints as it declares would
/// otherwise read as complete against a pattern count.
export function readCertifiedCoverage({
  catalogPath,
  packageName,
  packageVersion,
  declaredEntrypoints,
  declaredWildcard = false
}) {
  const entrypoints = certifiedEntrypointsOf({ catalogPath, packageName, packageVersion });
  if (entrypoints === null) return null;
  return {
    declaredEntrypoints: typeof declaredEntrypoints === "number" ? declaredEntrypoints : null,
    declaredWildcard: declaredWildcard === true,
    certifiedEntrypoints: entrypoints.length,
    rootCertified: entrypoints.includes(".")
  };
}

/// Whether this row's coverage is a measurement at all.
///
/// A coverage object with an unknown denominator (`declaredEntrypoints: null`,
/// an unreadable manifest) carries an exact certified count and no ratio, so it
/// belongs in neither half of the split: it is `coverage unmeasured`, exactly
/// like an unreadable catalog. Calling it partial would assert a shortfall
/// against a number nobody read.
export function isMeasuredCoverage(coverage) {
  return (
    coverage !== null &&
    typeof coverage === "object" &&
    typeof coverage.declaredEntrypoints === "number"
  );
}

/// Whether a certified row covered every entrypoint its manifest declares,
/// root included -- the `verified-complete` half of the split.
///
/// Deliberately conservative in three directions, because this is the value a
/// corpus-wide rate is computed from: an unmeasured coverage, an unknown
/// declared count, and an uncertified root all answer `false`. A row can only
/// be called complete when both numbers are known, they agree, and the root is
/// among them.
///
/// `declaredEntrypoints === 0` is the legacy-`main` package: it declares no
/// `exports` map at all, so its entire published surface *is* the root, and a
/// certified root covers it. That is a manifest shape, not a missing
/// measurement -- `readDeclaredEntrypointCensus` returns `null`, never `0`, when
/// it could not read the manifest.
///
/// A wildcard subpath (`"./src/*": "./src/*"`) makes the manifest's count a
/// *pattern* count rather than an entrypoint count: one declared entry expands
/// to as many real entrypoints as the package ships. `@kobalte/utils@0.9.2`
/// declares 2 and certifies 20. There is then no denominator to be complete
/// against, so `declaredWildcard` refuses completeness outright -- refusing is
/// the conservative direction, and `>= declared` would have called a wildcard
/// package with its root and one subpath fully verified.
///
/// The flag is what decides that, not `certified > declared`: a wildcard whose
/// expansion happens to certify exactly as many entrypoints as the manifest
/// declares is the same non-ratio as one that certifies ten times as many, and
/// coincidence is not a measurement. The count comparison is kept as well, for
/// a report recorded before the flag existed.
export function isCompleteCoverage(coverage) {
  if (
    !isMeasuredCoverage(coverage) ||
    coverage.rootCertified !== true ||
    !hasUsableDenominator(coverage)
  ) {
    return false;
  }
  return coverage.declaredEntrypoints === 0
    ? coverage.certifiedEntrypoints >= 1
    : coverage.certifiedEntrypoints === coverage.declaredEntrypoints;
}

/// Whether this row's declared count can serve as a denominator at all.
///
/// False when the manifest declares a wildcard subpath, and false when it
/// declares fewer entrypoints than the receipt covers -- which only a wildcard
/// produces, and which is the only evidence available on a report recorded
/// before `declaredWildcard` was measured. Reported rather than hidden:
/// `20 of 2` is not a ratio, and printing it as one would read as a bug in the
/// measurement instead of a fact about the manifest.
export function hasUsableDenominator(coverage) {
  return (
    isMeasuredCoverage(coverage) &&
    coverage.declaredWildcard !== true &&
    coverage.certifiedEntrypoints <= Math.max(coverage.declaredEntrypoints, 1)
  );
}
