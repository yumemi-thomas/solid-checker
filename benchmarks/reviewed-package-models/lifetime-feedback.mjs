import { runtimeFeedback as coreFeedback } from './extended-runtime-feedback.mjs';
import { lifetimeAudit } from './lifetime-audit.mjs';
export function runtimeFeedback(observe, options) {
  const base = coreFeedback(observe, options), audit = lifetimeAudit(globalThis, options);
  globalThis.__resourceAudit = audit;
  return { ...base, stop() { audit.stop(); base.stop(); }, resourceAudit: audit };
}
