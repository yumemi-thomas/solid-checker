// freeze-judge.mjs's refusal (ADR 0995): a frozen file that holds rows or `changes` is never overwritten by a regeneration.
//
// judge-frozen.json is regenerated from the CURRENT judge, so running the generator over it records whatever the judge now says as the
// frozen truth: a judge change that turned a false clean into `fine` would rewrite the row that failed, and judge-frozen.test.mjs, which
// compares the judge with the file, would pass. The file changes by hand, row by row, with a `changes` row (ADR, program) per moved id
// (scripts/guard/base.mjs holds that against the merge base).
/** A sentence when `existing` (the target file's text, or null when it is absent) must not be overwritten, else null. */
export function overwriteProblem(existing) {
  if (existing === null) return null;
  let parsed;
  try {
    parsed = JSON.parse(existing);
  } catch {
    return "the target exists and is not JSON: it is not overwritten";
  }
  const held = ["rows", "siteRows", "legalRows", "patternClaimRows", "changes"].filter((list) => (parsed[list] ?? []).length > 0);
  return held.length ? `the target already holds ${held.join(", ")}: a regeneration would record the current judge's answers as the frozen ones. Add or change rows by hand, with a \`changes\` row (ADR, program) per moved id; write a regeneration to a new path to diff it` : null;
}
