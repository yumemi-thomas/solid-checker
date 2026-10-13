// Local return hints require an admitted value path, not just a returned member.
import { ts } from './lower.mjs';
import { classSnapshotFlowsV4, getterSnapshotsV8, snapshotFeedbackV8 } from './snapshot-feedback-v8.mjs';
export { classSnapshotFlowsV4 };
export const snapshotFeedbackV9 = snapshotFeedbackV8;
export function getterSnapshotsV9(program, source, events) {
  const result = getterSnapshotsV8(program, source, events), candidates = [];
  for (const candidate of result.candidates) {
    if (!candidate.localReturn) { candidates.push(candidate); continue; }
    let returned = null;
    const find = node => {
      if (node.getStart(source) === candidate.localReturn.returnStart && node.end === candidate.localReturn.returnEnd) returned = node;
      if (!returned) ts.forEachChild(node, find);
    }; find(source);
    let unsupported = !returned;
    const check = node => {
      if (ts.isCallExpression(node) || ts.isNewExpression(node) || ts.isFunctionLike(node)) unsupported = true;
      ts.forEachChild(node, check);
    }; if (returned) check(returned);
    if (unsupported) result.open.push({ start: candidate.start, reason: 'local returned member has an unknown call or deferred value path' });
    else candidates.push(candidate);
  }
  return { ...result, candidates };
}
