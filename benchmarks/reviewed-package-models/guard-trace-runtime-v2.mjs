// A note about an executed guard, not an automatic bug warning. Intentional
// snapshots and background lifetimes can execute exactly the same paths.
// Keep distinct consumer caller stacks; a helper can run in several contexts.
export function collectGuardTrace(isAppFrame) {
  const events = [], seen = new Set();
  return { events, record(premise) {
    const previous = Error.stackTraceLimit;
    let stack; try { Error.stackTraceLimit = 100; stack = new Error().stack ?? ''; } finally { Error.stackTraceLimit = previous; }
    const frames = stack.split('\n').flatMap(line => { const match = line.match(/(?:at .*?\()?((?:file:\/\/|https?:\/\/|\/).*?):(\d+):(\d+)\)?$/); return match ? [{ path: match[1], line: Number(match[2]), column: Number(match[3]) }] : []; });
    const location = frames.find(isAppFrame) ?? null, key = JSON.stringify([premise.path, premise.start, premise.kind, frames.filter(isAppFrame)]);
    if (seen.has(key)) return; seen.add(key);
    events.push({ ...premise, location, frames, severity: 'info', basis: 'runtime-guard-observation', certification: false });
  } };
}
