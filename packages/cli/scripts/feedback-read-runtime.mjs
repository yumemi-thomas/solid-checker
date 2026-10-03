// Browser-only collector. Normal reads keep their value and execution count.
// Collection is bounded; missing records never establish correctness.
export function createReadCollector({ maxEvents = 256, maxBytes = 1024 * 1024 } = {}) {
  const events = [], nodes = new WeakMap();
  let nextNode = 0, intent = 0, bytes = 0, dropped = 0, reads = 0, queries = 0, inspecting = 0, current = null;
  function frames(skip) {
    const error = new Error();
    if (typeof Error.captureStackTrace === "function") Error.captureStackTrace(error, skip);
    return (error.stack ?? "").split("\n").slice(0, 40).flatMap(line => {
      const match = line.match(/(?:at .*?\()?((?:https?:\/\/|file:\/\/|\/).*?):(\d+):(\d+)\)?$/);
      return match ? [{ path: match[1], line: Number(match[2]), column: Number(match[3]) }] : [];
    });
  }
  function retain(ticket) {
    const size = new TextEncoder().encode(JSON.stringify(ticket)).length;
    if (events.length < maxEvents && bytes + size <= maxBytes) { events.push(ticket); bytes += size; }
    else dropped++;
  }
  return {
    events,
    get stats() { return { reads, observerQueries: queries, retained: events.length, dropped, bytes, complete: false }; },
    enterFunction(encoded, parent) {
      const model = JSON.parse(encoded), inherited = current ?? parent;
      const origin = model.derivedOrigin ? { path: model.path, sourceSha256: model.sourceSha256, span: model.derivedOrigin } : inherited?.origin;
      if (!origin) return null;
      const steps = model.derivedOrigin ? [] : [...(inherited?.steps ?? [])];
      if (!model.derivedOrigin && !current && parent?.origin) steps.push({ kind: "allocation", path: model.path,
        sourceSha256: model.sourceSha256, span: model.span, function: model.parent });
      return { origin, steps: steps.slice(0, 32), truncated: model.derivedOrigin ? false :
        (inherited?.truncated ?? false) || steps.length > 32 };
    },
    withOperation(token, encoded, invoke) {
      const previous = current;
      if (token) {
        const steps = [...token.steps, JSON.parse(encoded)];
        current = { origin: token.origin, steps: steps.slice(0, 32), truncated: token.truncated || steps.length > 32 };
      } else current = null;
      try { return invoke(); } finally { current = previous; }
    },
    enterIntent() { const previous = intent; intent++; return previous; },
    leaveIntent(previous) { intent = previous; },
    begin(node, getObserver, getOwner) {
      reads++;
      inspecting++;
      try { if (intent || getObserver() || getOwner() || !node || typeof node !== "object") return null; }
      finally { inspecting--; }
      if (!nodes.has(node)) nodes.set(node, ++nextNode);
      return { kind: "untracked-read", nodeId: nodes.get(node), frames: frames(this.begin), observer: false, owner: false,
        lineage: current ? { origin: current.origin, steps: [...current.steps], truncated: current.truncated } : null };
    },
    observerQuery(observer, getOwner) {
      queries++;
      if (!inspecting && !intent && observer === null && current?.origin && !getOwner()) {
        retain({ kind: "observer-query", nodeId: null, frames: frames(this.observerQuery), observer: false, owner: false,
          lineage: { origin: current.origin, steps: [...current.steps], truncated: current.truncated } });
      }
      return observer;
    },
    finish(value, ticket) {
      if (ticket) retain(ticket);
      return value;
    }
  };
}
export const reads = createReadCollector();
globalThis.__solidCheckerReads = reads;
