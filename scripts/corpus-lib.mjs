// Pure helpers shared by corpus-sweep.mjs and corpus-metrics.mjs: the dev /
// holdout split and the reviewed-findings ledger. No I/O, so vitest can drive
// them with small synthetic reports (scripts/corpus-lib.test.mjs).
import { relative } from "node:path";

export const SPLITS = ["dev", "holdout", "all"];

/** Manifest rows for a split. Every row must carry a boolean `holdout`. */
export function selectRows(manifest, split) {
  if (!SPLITS.includes(split)) throw new Error(`--split must be one of ${SPLITS.join(", ")}, got ${split}`);
  for (const row of manifest.projects) {
    if (typeof row.holdout !== "boolean") throw new Error(`manifest row ${row.app} ${row.project} has no boolean holdout`);
  }
  return manifest.projects.filter((row) => split === "all" || row.holdout === (split === "holdout"));
}

/** "app|project" -> holdout flag, for reports whose rows predate the `holdout` field. */
export function holdoutIndex(manifest) {
  return new Map(manifest.projects.map((row) => [`${row.app}|${row.project}`, row.holdout]));
}

/** A sweep row's split: its own flag, else the manifest's, else null (unknown). */
export function splitOf(result, index) {
  const flag = typeof result.holdout === "boolean" ? result.holdout : index?.get(`${result.app}|${result.project}`);
  return typeof flag === "boolean" ? (flag ? "holdout" : "dev") : null;
}

/**
 * The ledger key: corpus-sweep-compare.mjs's (app, project, id, kind, path,
 * startByte, endByte) with the path relative to the project root, so a key
 * does not depend on where the corpus is checked out.
 */
export function findingKey(result, finding) {
  const location = finding.primaryLocation ?? {};
  const path = location.path === undefined ? undefined : relative(result.root, location.path);
  return [result.app, result.project, finding.id, finding.kind, path, location.startByte, location.endByte].join("|");
}

/** Violation, uncertifiable, style or notice: the buckets corpus-metrics.mjs counts. */
export function bucketOf(finding) {
  return finding.subjectKind === "project" ? "notice"
    : /^SC80/.test(finding.id) ? "style"
      : finding.kind === "violation" ? "violation" : "uncertifiable";
}

export const CONFIRMED_CLASSES = ["confirmed-violation", "adopted-policy-violation"];
export const LEDGER_CLASSES = [
  ...CONFIRMED_CLASSES, "valid-for-claim", "essential-unknown", "unresolved-review", "tsc-owned", "compiler-refused",
];
const KEY_PARTS = 7;

/** Problems with a ledger document; empty when it is well formed. */
export function validateLedger(ledger) {
  const errors = [];
  if (ledger === null || typeof ledger !== "object" || Array.isArray(ledger)) return ["ledger must be an object"];
  if (!Array.isArray(ledger.entries)) return ["ledger.entries must be a list"];
  const seen = new Set();
  ledger.entries.forEach((entry, index) => {
    const where = `entries[${index}]`;
    if (entry === null || typeof entry !== "object" || Array.isArray(entry)) return errors.push(`${where} must be an object`);
    const extra = Object.keys(entry).filter((name) => !["key", "class", "reviewer", "date", "evidence"].includes(name));
    if (extra.length) errors.push(`${where} has unknown fields ${extra.join(", ")}`);
    const parts = typeof entry.key === "string" ? entry.key.split("|") : [];
    if (parts.length !== KEY_PARTS || parts.slice(0, 4).some((part) => part === "") || !parts.slice(5).every((part) => /^\d+$/.test(part))) {
      errors.push(`${where}.key must be app|project|id|kind|path|startByte|endByte`);
    } else if (seen.has(entry.key)) errors.push(`${where}.key is listed twice: ${entry.key}`);
    else seen.add(entry.key);
    if (!LEDGER_CLASSES.includes(entry.class)) errors.push(`${where}.class ${JSON.stringify(entry.class)} is not one of ${LEDGER_CLASSES.join(", ")}`);
    if (typeof entry.reviewer !== "string" || !entry.reviewer.trim()) errors.push(`${where}.reviewer must be a non-empty string`);
    if (typeof entry.date !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(entry.date) || Number.isNaN(Date.parse(entry.date))) {
      errors.push(`${where}.date must be YYYY-MM-DD`);
    }
    if (typeof entry.evidence !== "string" || !entry.evidence.trim() || entry.evidence.includes("\n")) {
      errors.push(`${where}.evidence must be a one-line, non-empty string`);
    }
  });
  return errors;
}

const emptyShare = () => ({ violation: 0, uncertifiable: 0, style: 0, notice: 0 });
export const decidedShare = (bucket) => {
  const decided = bucket.violation + bucket.uncertifiable;
  return decided === 0 ? null : bucket.violation / decided;
};

/**
 * Decided share per split. `index` is the manifest's holdout lookup, used for
 * rows with no `holdout` flag of their own.
 */
export function shareBySplit(results, index) {
  const total = emptyShare();
  const splits = { dev: emptyShare(), holdout: emptyShare() };
  for (const result of results) {
    const split = splitOf(result, index);
    for (const finding of result.findings ?? []) {
      const slot = bucketOf(finding);
      total[slot] += 1;
      if (split) splits[split][slot] += 1;
    }
  }
  return { total, dev: splits.dev, holdout: splits.holdout };
}

/**
 * Reviewed precision: confirmed / adjudicated violations. Only violations that
 * a ledger entry classifies are adjudicated; an unresolved-review entry is not
 * a decision, so it is neither confirmed nor adjudicated. Unreviewed violations
 * are counted apart and never enter the ratio. A ledger entry whose project ran
 * in this sweep but whose key matches no finding is stale (reported, not
 * dropped); an entry for a project this sweep did not run is out of scope.
 */
export function reviewedPrecision(results, ledger, index) {
  const classOf = new Map(ledger.entries.map((entry) => [entry.key, entry.class]));
  const present = new Set();
  const ran = new Set(results.map((result) => `${result.app}|${result.project}`));
  const empty = () => ({ confirmed: 0, adjudicated: 0, unresolved: 0, unreviewed: 0, byClass: {} });
  const total = empty();
  const splits = { dev: empty(), holdout: empty() };
  let otherKind = 0;
  for (const result of results) {
    const split = splitOf(result, index);
    for (const finding of result.findings ?? []) {
      const key = findingKey(result, finding);
      present.add(key);
      if (classOf.has(key) && bucketOf(finding) !== "violation") {
        otherKind += 1;
        continue;
      }
      if (bucketOf(finding) !== "violation") continue;
      const cls = classOf.get(key);
      for (const bucket of [total, split && splits[split]].filter(Boolean)) {
        if (cls === undefined) bucket.unreviewed += 1;
        else if (cls === "unresolved-review") bucket.unresolved += 1;
        else {
          bucket.adjudicated += 1;
          if (CONFIRMED_CLASSES.includes(cls)) bucket.confirmed += 1;
          bucket.byClass[cls] = (bucket.byClass[cls] ?? 0) + 1;
        }
      }
    }
  }
  const stale = ledger.entries
    .filter((entry) => !present.has(entry.key) && ran.has(entry.key.split("|").slice(0, 2).join("|")))
    .map((entry) => entry.key);
  const withPrecision = (bucket) => ({ ...bucket, precision: bucket.adjudicated === 0 ? null : bucket.confirmed / bucket.adjudicated });
  return {
    total: withPrecision(total), dev: withPrecision(splits.dev), holdout: withPrecision(splits.holdout),
    stale, otherKind,
  };
}
