// Join executed records to native, exact-source models. No package names,
// API spellings, expected values, comparison code or benchmark roles enter
// selection. Candidate result relationships were computed by the native IR.
export function selectReadFeedback(events, models, { typingErrors = 0, dropped = 0 } = {}) {
  const notes = [], queries = [], open = [], seen = new Set();
  if (typingErrors) return { notes, open, excluded: "TypeScript owns this input", authority: false, certification: false };
  const files = new Map(models.map(file => [file.path, file]));
  const same = (left, right) => left && right && left.start === right.start && left.end === right.end;
  for (const event of events) {
    const lineage = event.lineage;
    if (lineage && (!Array.isArray(lineage.steps) || lineage.steps.length > 32)) {
      open.push({ reason: "Malformed or over-budget callback lineage", nodeId: event.nodeId }); continue;
    }
    if (!lineage?.origin) { open.push({ reason: "Derived origin was not observed", nodeId: event.nodeId }); continue; }
    const origin = lineage.origin, file = files.get(origin.path);
    if (!file || file.sourceSha256 !== origin.sourceSha256 ||
      !file.functions.some(fn => same(fn.derivedOrigin, origin.span))) {
      open.push({ reason: "Derived origin does not bind current native facts", nodeId: event.nodeId }); continue;
    }
    let valid = !lineage.truncated && lineage.steps.length > 0, returned = true;
    for (const step of lineage.steps) {
      const source = files.get(step.path);
      let relevance;
      if (step.kind === "allocation") {
        const fn = source?.functions.find(row => same(row.span, step.span) && same(row.parent, step.function));
        relevance = fn?.allocationRelevance;
      } else {
        const operation = source?.operations.find(row => same(row.span, step.span) && same(row.function, step.function));
        const fn = source?.functions.find(row => same(row.span, step.function));
        relevance = fn && operation?.resultRelevance;
      }
      if (!source || source.sourceSha256 !== step.sourceSha256 || !relevance) { valid = false; break; }
      returned &&= ["return-expression", "local-initializer", "distinct-branch"].includes(relevance);
    }
    if (!valid || !returned) {
      open.push({ origin, nodeId: event.nodeId, reason: !valid ? "Incomplete or stale callback lineage" :
        "Result relevance or competing return paths remain open" }); continue;
    }
    const site = event.site, last = lineage.steps.at(-1);
    if (!site || site.sourceSha256 !== last.sourceSha256 || site.location.path !== last.path ||
      site.location.startByte < last.span.start || site.location.endByte > last.span.end || last.kind === "allocation") {
      open.push({ origin, nodeId: event.nodeId, reason: "Observed authored site is outside the final modeled operation" }); continue;
    }
    // Node identities are per document; a full page load starts a new one.
    const identity = `${origin.path}:${origin.span.start}:${event.document ?? 0}:${event.nodeId}`;
    if (seen.has(identity)) continue; seen.add(identity);
    if (event.kind === "observer-query") {
      queries.push({ kind: "observer-query-outside-tracking", severity: "info", channel: "executed-development",
        message: "A call related to this derived result queried tracking and found no observer. A package may skip subscribing in this context; check whether its result should follow changing values.",
        origin, observedRead: event.site ?? null, lineage: lineage.steps, reactiveRead: "unproven", reactiveIntent: "open",
        authority: false, certification: false }); continue;
    }
    notes.push({ kind: "potential-untracked-result-read", severity: "warning", channel: "executed-development",
      message: "A callback reached a reactive read without an observer. Native source models connect its call chain to a possible result of a derived computation. If this result should follow the value, read it while tracking and pass the captured value to the deferred work.",
      origin, nodeId: event.nodeId, lineage: lineage.steps, reactiveIntent: "open",
      observedRead: event.site ?? null,
      resultFlow: "native-value-flow-candidate", promiseSettlement: "unproven", authority: false, certification: false });
  }
  notes.push(...queries.filter(query => !notes.some(note => same(note.origin.span, query.origin.span) && note.origin.path === query.origin.path)));
  if (dropped) open.push({ reason: "Read evidence exceeded the retention budget", dropped });
  return { notes, open, authority: false, certification: false, complete: false };
}
