// Combine independent observations. Authored expectations, target/control
// roles and package-specific recipes are not inputs to this feedback function.
export function familyFeedback(browser, statics = {}, projected = statics.warnings ?? []) {
  if (browser.publishedTypingErrors.length) return { excluded: 'TypeScript owns this input', feedback: [], gaps: [] };
  const feedback = [];
  for (const finding of statics.baseline?.findings ?? []) if (finding.kind === 'violation') feedback.push({
    channel: 'native-static', code: finding.id, rule: finding.rule, severity: finding.severity,
    basis: 'native analysis of original consumer', findingKind: finding.kind, certification: false,
    message: finding.message, location: finding.primaryLocation,
  });
  for (const item of projected) feedback.push({ ...item, channel: 'source-assumption' });
  for (const note of statics.getter?.notes ?? []) {
    const prefix = statics.originalText?.slice(0, note.start), lines = prefix?.split('\n');
    feedback.push({ ...note, channel: 'source-candidate', category: 'intent-open', location: prefix === undefined ? null : {
      path: statics.originalPath, startByte: Buffer.byteLength(prefix), line: lines.length, column: lines.at(-1).length + 1,
    } });
  }
  for (const item of browser.feedback ?? []) feedback.push({ ...item, channel: item.category === 'lifetime-expectation' ? 'declared-lifetime' : 'native-runtime',
    location: item.originalLocation ?? null });
  const errors = [...browser.errors ?? [], ...browser.pageErrors ?? [], ...browser.windowErrors ?? []];
  const seen = new Set();
  for (const error of errors) {
    const message = typeof error === 'string' ? error : error.message;
    const location = typeof error === 'string' ? null : error.originalLocation ?? null;
    const key = JSON.stringify([message, location]); if (seen.has(key)) continue; seen.add(key);
    feedback.push({ channel: 'runtime-exception', code: 'RUNTIME_EXCEPTION', severity: 'error', certification: false,
      category: 'execution', basis: 'observed exception; semantic rule unresolved', message, location });
  }
  return { excluded: null, feedback, gaps: [
    ...statics.baseline?.findings.filter(f => f.kind === 'uncertifiable') ?? [],
    ...statics.unsupported ?? [], ...statics.getter?.open ?? [],
  ] };
}
