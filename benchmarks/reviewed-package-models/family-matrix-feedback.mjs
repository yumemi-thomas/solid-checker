// A shared collector: native observations and explicit lifetime expectations.
// Case expectations and package names never select a diagnostic here.
import { runtimeFeedback as core } from './extended-runtime-feedback.mjs';
import { lifetimeAudit } from './lifetime-audit.mjs';
export function runtimeFeedback(observe, options) {
  const native = core(observe, options), audit = lifetimeAudit(globalThis, options);
  globalThis.__resourceAudit = audit;
  return { ...native, stop() { audit.stop(); native.stop(); }, resourceAudit: audit };
}
