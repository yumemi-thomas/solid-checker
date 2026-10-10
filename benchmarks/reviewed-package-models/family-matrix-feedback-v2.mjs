// Preserve each observed consumer call path while sharing lifetime monitoring.
import { runtimeFeedback as core } from './extended-runtime-feedback-v2.mjs';
import { lifetimeAudit } from './lifetime-audit.mjs';
export function runtimeFeedback(observe, options) {
  const native = core(observe, options), audit = lifetimeAudit(globalThis, options);
  globalThis.__resourceAudit = audit;
  return { ...native, stop() { audit.stop(); native.stop(); }, resourceAudit: audit };
}
