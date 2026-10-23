// Descriptive context fields of a runtime-oracle case (`oracle.json`): what
// execution the case stands for. They change no verdict; the report row
// carries them and the summary counts cases per scenario kind. A malformed
// field refuses its case (a harness-error), like a malformed `behaviour`.
import assert from "node:assert/strict";

export const SCENARIOS = ["application", "api-example", "constructed-misuse"];

/** The validated { scenario, requires, reaches } of a case spec; absent fields are null. */
export function contextSpec(id, spec) {
  assert(spec.scenario === undefined || SCENARIOS.includes(spec.scenario), `${id}: scenario is one of ${SCENARIOS.join(", ")}`);
  assert(spec.requires === undefined || (Array.isArray(spec.requires) && spec.requires.length > 0
    && spec.requires.every((item) => typeof item === "string" && item.trim())), `${id}: requires is a non-empty list of non-empty strings; omit the key`);
  assert(spec.reaches === undefined || (typeof spec.reaches === "string" && spec.reaches.trim()), `${id}: reaches is a non-empty string; omit the key`);
  return { scenario: spec.scenario ?? null, requires: spec.requires ?? null, reaches: spec.reaches ?? null };
}

/** Cases per scenario kind; a case with none is "unspecified". */
export function scenarioCounts(rows) {
  const counts = {};
  for (const row of rows) counts[row.scenario ?? "unspecified"] = (counts[row.scenario ?? "unspecified"] ?? 0) + 1;
  return counts;
}
