// Port of the experiment's replay-assertion-feedback-v1 decision boundary.
// The comparison is supplied and measured; no automatic repair is inferred.
export function selectAssertionFeedback(before, after) {
  const notes = [], open = [], unchanged = [];
  for (const assertion of before) {
    const comparison = after.find(row => row.id === assertion.id);
    if (!comparison || comparison.expected !== assertion.expected || comparison.selector !== assertion.selector) {
      open.push({ id: assertion.id, reason: "No matching measured comparison assertion" }); continue;
    }
    if (assertion.actual === assertion.expected) {
      unchanged.push({ id: assertion.id, passedBefore: true, passedAfter: comparison.actual === assertion.expected });
      if (comparison.actual !== assertion.expected) open.push({ id: assertion.id, reason: "Comparison breaks a previously passing assertion" });
      continue;
    }
    if (comparison.actual !== assertion.expected) {
      open.push({ id: assertion.id, reason: "Comparison does not satisfy the supplied assertion" }); continue;
    }
    notes.push({ id: assertion.id, code: "COMPARISON_SATISFIES_VALUE_ASSERTION", severity: "info",
      message: `The supplied comparison made this assertion pass (${assertion.actual} → ${comparison.actual}, expected ${assertion.expected}). Review execution timing and side effects.`,
      selector: assertion.selector, expected: assertion.expected, before: assertion.actual, after: comparison.actual,
      repairSafety: "unproved", reactiveIntent: "open", authority: false, certification: false,
      scope: "Only the supplied assertion under the executed interaction" });
  }
  return { notes, open, unchanged, authority: false, certification: false };
}
