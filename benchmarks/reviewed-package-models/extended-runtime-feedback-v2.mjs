// Additional exact-runtime channels, kept separate from the frozen breadth
// collector. Advisory graph observations are not proven application defects.
const executionCodes = new Set([
  "STRICT_READ_UNTRACKED", "PENDING_ASYNC_UNTRACKED_READ", "PENDING_ASYNC_FORBIDDEN_SCOPE",
  "REACTIVE_WRITE_IN_OWNED_SCOPE", "NO_OWNER_CLEANUP", "CLEANUP_IN_FORBIDDEN_SCOPE",
  "NO_OWNER_EFFECT", "PRIMITIVE_IN_FORBIDDEN_SCOPE", "ACTION_CALLED_IN_OWNED_SCOPE",
  "FLUSH_IN_EFFECT_CALLBACK", "RUN_WITH_DISPOSED_OWNER", "SETTLED_CLEANUP_UNOWNED", "NO_OWNER_BOUNDARY",
]);
const advisoryCodes = new Set([
  "ASYNC_OUTSIDE_LOADING_BOUNDARY", "EFFECT_WRITES_OWN_SOURCE", "EFFECT_RELAY_TEAR",
  "UNSTABLE_MEMO_OUTPUT", "IMMUTABLE_UPDATE_IN_STORE", "UNSTABLE_LIST_IDENTITY",
]);
export function runtimeFeedback(observe, { appRoot, isAppFrame, onFeedback = () => {} }) {
  if (!observe?.diagnostics?.subscribe) throw new Error("This runtime has no diagnostic subscription channel");
  const feedback = [], seen = new Map();
  const accepts = frame => isAppFrame ? isAppFrame(frame) :
    (frame.path.startsWith(appRoot + "/") || frame.path.startsWith("file://" + appRoot + "/")) && !frame.path.includes("/node_modules/");
  const stop = observe.diagnostics.subscribe(event => {
    if (!executionCodes.has(event.code) && !advisoryCodes.has(event.code)) return;
    const prior = Error.stackTraceLimit;
    let stack; try { Error.stackTraceLimit = 100; stack = new Error().stack ?? ""; } finally { Error.stackTraceLimit = prior; }
    const frames = stack.split("\n").flatMap(line => {
      const match = line.match(/(?:at .*?\()?((?:file:\/\/|https?:\/\/|\/).*?):(\d+):(\d+)\)?$/);
      return match ? [{ path: match[1], line: Number(match[2]), column: Number(match[3]) }] : [];
    });
    const registration = globalThis.__packageOrigins?.current() ?? null;
    const location = frames.find(accepts) ?? null;
    const consumerFrames = frames.filter(accepts);
    const key = JSON.stringify([event.code, consumerFrames, registration?.premise]);
    const priorItem = seen.get(key);
    if (priorItem) { priorItem.occurrences++; return; }
    const item = { code: event.code, severity: event.severity, message: event.message, kind: event.kind,
      ownerPath: event.ownerPath ?? [], data: event.data ?? null, basis: "runtime-observation", certification: false,
      category: advisoryCodes.has(event.code) ? "advisory" : "execution", location, frames, consumerFrames, occurrences: 1, registration };
    seen.set(key, item); feedback.push(item); onFeedback(item);
  });
  return { feedback, stop, clear() { feedback.length = 0; seen.clear(); } };
}
