// The dialect negative authority's reach over the corpus.
//
// A negative row is admitted only after an identity gate binds the installed
// archive to an audited one field by field -- name, version, integrity and the
// archive's own `package.json` digest
// (`contract_certification/type_facts.rs::audited_archive_for_snapshot`). The
// authority is pinned to the exact prereleases it was read against, so its
// reach over a corpus is whatever fraction of that corpus installs those
// bytes, and that fraction falls to nothing on the next release of the audited
// package -- silently, because an unmatched identity produces the same refusal
// a withheld claim produces.
//
// This module is what makes the fraction a number. It reads the pins from
// `rust/crates/solid-dialect/audited-archives.json` -- mirrored out of the Rust
// tables and held to them by `audited_archives_json_mirrors_the_dialect_tables`
// -- and joins them against what each probe row installed.
//
// **It is an upper bound, and never a yield.** The join is on name and version
// only: a report records installed versions, not integrity or manifest
// digests, so a row counted here is one the identity gate *could* reach, not
// one it did. A row it does not count is one the gate cannot reach at all,
// which is the direction the number is for.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const AUDITED_ARCHIVES_PATH = fileURLToPath(
  new URL("../../../rust/crates/solid-dialect/audited-archives.json", import.meta.url)
);

/// The dialect a probe row's `solidTarget` speaks, in the assembly manifests'
/// spelling. An unrecognized target maps to nothing rather than to a default:
/// attributing a row to the wrong authority would count coverage it never had.
const DIALECT_OF_TARGET = new Map([
  ["solid1", "solid-v1"],
  ["solid2", "solid-v2"]
]);

/// The pinned archives, or a throw.
///
/// Deliberately strict about shape. A file that parses but carries no
/// `dialects` array would otherwise report full coverage of an empty pin set,
/// which reads as "nothing is wrong" -- the exact failure this module exists to
/// make loud.
export function loadAuditedArchives(path = AUDITED_ARCHIVES_PATH) {
  const document = JSON.parse(readFileSync(path, "utf8"));
  if (document?.schemaVersion !== 1) {
    throw new Error(`${path}: unsupported schemaVersion ${document?.schemaVersion}`);
  }
  if (!Array.isArray(document.dialects) || document.dialects.length === 0) {
    throw new Error(`${path}: no dialects`);
  }
  for (const dialect of document.dialects) {
    if (typeof dialect?.id !== "string" || !dialect.id) {
      throw new Error(`${path}: a dialect has no id`);
    }
    if (!Array.isArray(dialect.archives)) {
      throw new Error(`${path}: ${dialect.id} has no archives array`);
    }
    for (const archive of dialect.archives) {
      if (typeof archive?.name !== "string" || typeof archive?.version !== "string") {
        throw new Error(`${path}: ${dialect.id} carries an archive with no name@version`);
      }
    }
  }
  return document;
}

function percentage(part, whole) {
  if (!whole) return null;
  return Math.round((part / whole) * 1000) / 10;
}

/// How much of this run's corpus the negative authority can answer about.
///
/// `rowsCovered` counts probe rows that installed at least one archive the
/// row's **own** dialect audited, at the audited version. Keyed by the row's
/// dialect on purpose: an authority says nothing about a package a different
/// dialect audited under the same name, and the identity gate agrees -- an
/// authority participates only when its own archive list carries the exact
/// tuple.
export function buildDialectAuthorityCoverage(results, auditedArchives) {
  const rows = (Array.isArray(results) ? results : []).filter(
    result => result?.status !== "supplemental"
  );

  // Three indexes over the same pins: what this row's dialect audited, which
  // names are audited at all (the only installs worth listing), and which
  // exact tuples are (so a listed version says whether it is the pin).
  const versionsByDialectAndName = new Map();
  const auditedNames = new Set();
  const auditedTuples = new Set();
  const byDialect = new Map();
  for (const dialect of auditedArchives.dialects) {
    const names = new Map();
    versionsByDialectAndName.set(dialect.id, names);
    byDialect.set(dialect.id, {
      id: dialect.id,
      auditedArchives: dialect.archives.length,
      negativeRowCount: dialect.negativeRowCount ?? null,
      rows: 0,
      rowsCovered: 0
    });
    for (const archive of dialect.archives) {
      if (!names.has(archive.name)) names.set(archive.name, new Set());
      names.get(archive.name).add(archive.version);
      auditedNames.add(archive.name);
      auditedTuples.add(`${archive.name}@${archive.version}`);
    }
  }

  const installed = new Map();
  let rowsCovered = 0;
  let rowsWithoutDialect = 0;
  for (const row of rows) {
    const dialectId = DIALECT_OF_TARGET.get(row?.solidTarget);
    if (!dialectId) {
      rowsWithoutDialect += 1;
      continue;
    }
    const summary = byDialect.get(dialectId);
    if (summary) summary.rows += 1;
    const auditedByThisDialect = versionsByDialectAndName.get(dialectId) ?? new Map();
    const versions = row?.installedVersions;
    let covered = false;
    for (const [name, version] of Object.entries(
      versions && typeof versions === "object" ? versions : {}
    )) {
      // Only packages some dialect audited under this name are interesting;
      // the rest of an install tree says nothing about the authority's reach.
      if (typeof version !== "string" || !auditedNames.has(name)) continue;
      const key = `${name}@${version}`;
      const entry = installed.get(key) ?? {
        package: name,
        version,
        rows: 0,
        audited: auditedTuples.has(key)
      };
      entry.rows += 1;
      installed.set(key, entry);
      if (auditedByThisDialect.get(name)?.has(version)) covered = true;
    }
    if (covered) {
      rowsCovered += 1;
      if (summary) summary.rowsCovered += 1;
    }
  }

  return {
    // The pins themselves, so a report says which bytes the number is about
    // rather than leaving the reader to open a Rust file.
    archives: auditedArchives.dialects.flatMap(dialect =>
      dialect.archives.map(archive => ({
        dialect: dialect.id,
        package: archive.name,
        version: archive.version
      }))
    ),
    rows: rows.length,
    rowsCovered,
    coveragePercentage: percentage(rowsCovered, rows.length),
    rowsWithoutDialect,
    byDialect: [...byDialect.values()],
    // Every installed version of an audited *name*, ranked, so the versions
    // the pin misses are named rather than implied by the shortfall.
    installedVersions: [...installed.values()].sort(
      (left, right) =>
        right.rows - left.rows ||
        left.package.localeCompare(right.package) ||
        left.version.localeCompare(right.version)
    )
  };
}
