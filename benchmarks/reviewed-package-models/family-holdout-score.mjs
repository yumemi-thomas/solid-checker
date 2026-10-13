// Evaluation only: detector inputs do not include these expectations.
export function matchingClaims(row) {
  const { rules = [], codes = [] } = row.provenance;
  return row.feedback.filter(item => rules.includes(item.rule) || codes.includes(item.code));
}
export function scoreHoldout(rows) {
  const valid = rows.filter(row => !row.excluded), targets = valid.filter(row => row.provenance.role === 'target'),
    controls = valid.filter(row => row.provenance.role === 'control');
  const complete = row => !row.harnessFailure;
  const quiet = row => complete(row) && !row.feedback.length && row.declaredBehavior?.passed !== false;
  const caught = row => complete(row) && matchingClaims(row).length > 0;
  const pairs = targets.map(target => {
    const paired = controls.filter(control => control.provenance.pair === target.provenance.pair);
    return { pair: target.provenance.pair, target: target.id, controls: paired.map(row => row.id),
      matched: !!caught(target), controlsQuietAndCorrect: paired.length > 0 && paired.every(quiet),
      passed: !!caught(target) && paired.length > 0 && paired.every(quiet) };
  });
  return { records: rows.length, executed: valid.length, typeExcluded: rows.length - valid.length,
    targets: targets.length, matchedTargets: targets.filter(caught).length,
    targetMisses: targets.filter(row => !caught(row)).map(row => row.id),
    unrelatedFeedbackTargets: targets.filter(row => row.feedback.length && !matchingClaims(row).length).map(row => row.id),
    controls: controls.length, quietCorrectControls: controls.filter(quiet).length,
    noisyControls: controls.filter(row => row.feedback.length).map(row => row.id),
    controlBehaviorFailures: controls.filter(row => row.declaredBehavior?.passed === false).map(row => row.id),
    harnessFailures: valid.filter(row => row.harnessFailure).map(row => row.id),
    correctlyHandled: targets.filter(caught).length + controls.filter(quiet).length,
    pairsPassed: pairs.filter(row => row.passed).length, pairs,
    byPackage: [...new Set(rows.map(row => row.package))].map(name => {
      const selected = valid.filter(row => row.package === name), target = selected.filter(row => row.provenance.role === 'target'),
        control = selected.filter(row => row.provenance.role === 'control');
      return { package: name, targets: target.length, matchedTargets: target.filter(caught).length,
        controls: control.length, quietCorrectControls: control.filter(quiet).length };
    }) };
}
