// Browser-only collector. Normal reads keep their value and execution count.
// Collection is bounded; missing records never establish correctness.
// The synchronous stack is captured up to `maxFrames`; a record whose stack
// reaches that depth says so, because a missing application frame below it
// cannot then be distinguished from one that was never there.
export function createReadCollector({ maxEvents = 256, maxBytes = 1024 * 1024, maxFrames = 40, maxScopes = 4096 } = {}) {
  const events = [], nodes = new WeakMap(), scopes = new Map();
  let nextNode = 0, intent = 0, bytes = 0, dropped = 0, reads = 0, queries = 0, inspecting = 0, current = null, scopesDropped = 0;
  function frames(skip) {
    const limit = Error.stackTraceLimit;
    Error.stackTraceLimit = maxFrames + 1;
    let error;
    try {
      error = new Error();
      if (typeof Error.captureStackTrace === "function") Error.captureStackTrace(error, skip);
    } finally { Error.stackTraceLimit = limit; }
    const rows = (error.stack ?? "").split("\n").flatMap(line => {
      const match = line.match(/(?:at .*?\()?((?:https?:\/\/|file:\/\/|\/).*?):(\d+):(\d+)\)?$/);
      return match ? [{ path: match[1], line: Number(match[2]), column: Number(match[3]) }] : [];
    });
    return { frames: rows.slice(0, maxFrames), truncated: rows.length > maxFrames };
  }
  function record(kind, nodeId, skip, lineage) {
    const stack = frames(skip);
    return { kind, nodeId, frames: stack.frames, stackTruncated: stack.truncated, observer: false, owner: false, lineage };
  }
  // Execution counts for instrumented native candidate scopes, keyed by exact
  // source span. A scope absent here was never entered in this run.
  function scope(kind, model) {
    if (typeof model?.span?.start !== "number" || typeof model.span.end !== "number") return null;
    const id = `${kind}:${model.path}:${model.span.start}:${model.span.end}`;
    let row = scopes.get(id);
    if (!row) {
      if (scopes.size >= maxScopes) { scopesDropped++; return null; }
      row = { kind, path: model.path, sourceSha256: model.sourceSha256, span: model.span, entered: 0,
        ...(kind === "operation" ? { returned: 0, threw: 0 } : {}) };
      scopes.set(id, row);
    }
    return row;
  }
  function retain(ticket) {
    const size = new TextEncoder().encode(JSON.stringify(ticket)).length;
    if (events.length < maxEvents && bytes + size <= maxBytes) { events.push(ticket); bytes += size; }
    else dropped++;
  }
  return {
    events,
    get stats() { return { reads, observerQueries: queries, retained: events.length, dropped, bytes, complete: false }; },
    get scopes() { return { rows: [...scopes.values()].map(row => ({ ...row })), dropped: scopesDropped }; },
    enterFunction(encoded, parent) {
      const model = JSON.parse(encoded), inherited = current ?? parent, counted = scope("function", model);
      if (counted) counted.entered++;
      const origin = model.derivedOrigin ? { path: model.path, sourceSha256: model.sourceSha256, span: model.derivedOrigin } : inherited?.origin;
      if (!origin) return null;
      const steps = model.derivedOrigin ? [] : [...(inherited?.steps ?? [])];
      if (!model.derivedOrigin && !current && parent?.origin) steps.push({ kind: "allocation", path: model.path,
        sourceSha256: model.sourceSha256, span: model.span, function: model.parent });
      return { origin, steps: steps.slice(0, 32), truncated: model.derivedOrigin ? false :
        (inherited?.truncated ?? false) || steps.length > 32 };
    },
    withOperation(token, encoded, invoke) {
      const previous = current, operation = JSON.parse(encoded), counted = scope("operation", operation);
      if (token) {
        const steps = [...token.steps, operation];
        current = { origin: token.origin, steps: steps.slice(0, 32), truncated: token.truncated || steps.length > 32 };
      } else current = null;
      if (counted) counted.entered++;
      // Synchronous completion only: a returned Promise counts as returned.
      try { const value = invoke(); if (counted) counted.returned++; return value; }
      catch (error) { if (counted) counted.threw++; throw error; }
      finally { current = previous; }
    },
    enterIntent() { const previous = intent; intent++; return previous; },
    leaveIntent(previous) { intent = previous; },
    begin(node, getObserver, getOwner) {
      reads++;
      inspecting++;
      try { if (intent || getObserver() || getOwner() || !node || typeof node !== "object") return null; }
      finally { inspecting--; }
      if (!nodes.has(node)) nodes.set(node, ++nextNode);
      return record("untracked-read", nodes.get(node), this.begin,
        current ? { origin: current.origin, steps: [...current.steps], truncated: current.truncated } : null);
    },
    observerQuery(observer, getOwner) {
      queries++;
      if (!inspecting && !intent && observer === null && current?.origin && !getOwner()) {
        retain(record("observer-query", null, this.observerQuery,
          { origin: current.origin, steps: [...current.steps], truncated: current.truncated }));
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
