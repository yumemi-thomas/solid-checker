// A development observer, not package certification. These codes express
// execution/ownership facts beyond the published TypeScript signatures.
const semanticCodes = new Set(["STRICT_READ_UNTRACKED", "PENDING_ASYNC_UNTRACKED_READ", "PENDING_ASYNC_FORBIDDEN_SCOPE",
  "REACTIVE_WRITE_IN_OWNED_SCOPE", "NO_OWNER_CLEANUP", "CLEANUP_IN_FORBIDDEN_SCOPE", "NO_OWNER_EFFECT",
  "PRIMITIVE_IN_FORBIDDEN_SCOPE", "ACTION_CALLED_IN_OWNED_SCOPE", "FLUSH_IN_EFFECT_CALLBACK", "RUN_WITH_DISPOSED_OWNER"]);

export function runtimeFeedback(observe, { appRoot, isAppFrame, onFeedback = () => {} }) {
  if (!observe?.diagnostics?.subscribe) throw new Error("This runtime has no diagnostic subscription channel");
  const feedback = [], seen = new Set();
  const stop = observe.diagnostics.subscribe(event => {
    if (!semanticCodes.has(event.code)) return;
    // Capture synchronously while the offending package/user frames are live.
    // Browser source maps are applied by the browser harness after collection.
    // Deep dependency callbacks must not truncate the original app entry frame.
    const previousLimit = Error.stackTraceLimit;
    let stack;
    try { Error.stackTraceLimit = 100; stack = new Error().stack ?? ""; }
    finally { Error.stackTraceLimit = previousLimit; }
    const frames = stack.split("\n").flatMap(line => {
      const match = line.match(/(?:at .*?\()?((?:file:\/\/|https?:\/\/|\/).*?):(\d+):(\d+)\)?$/);
      return match ? [{ path: match[1], line: Number(match[2]), column: Number(match[3]) }] : [];
    });
    const location = frames.find(frame => isAppFrame ? isAppFrame(frame) :
      (frame.path.startsWith(appRoot + "/") || frame.path.startsWith("file://" + appRoot + "/")) &&
      !frame.path.includes("/node_modules/") && !frame.path.includes("/runtime-feedback.mjs")) ?? null;
    const key = JSON.stringify([event.code, location]);
    if (seen.has(key)) return;
    seen.add(key);
    const item = { code: event.code, severity: event.severity, message: event.message, ownerPath: event.ownerPath ?? [],
      basis: "runtime-observation", certification: false, location, frames };
    feedback.push(item); onFeedback(item);
  });
  return { feedback, stop, clear() { feedback.length = 0; seen.clear(); } };
}
