// The project policy is explicit; this only supplies event/continuation context.
const backgrounds = new WeakSet(), owners = new WeakMap();
export const state = { registrations: [], gaps: [], ended: 0 };
globalThis.__automaticLifetimes = state;
export function background(callback) { backgrounds.add(callback); return callback; }
export function captureLifetime() { return globalThis.__resourceAudit?.capture() ?? null; }
export function ownerEvent(callback, getOwner, onCleanup, premise) {
  const audit = globalThis.__resourceAudit;
  if (!audit || callback == null || backgrounds.has(callback)) return callback;
  if (typeof callback !== 'function') { state.gaps.push({ premise, reason: 'Unsupported non-function DOM event handler' }); return callback; }
  const owner = getOwner();
  // Exact rc.9 ABI: plain createOwner objects initially omit _flags; teardown
  // writes it. Computations carry integer flags. The loader authenticates rc.9.
  const knownFlags = owner && (Number.isInteger(owner._flags) || owner._root === true && owner._flags === undefined);
  if (!owner || !Number.isInteger(owner._config) || !knownFlags || owner._config & 16 || owner._flags & 64) {
    state.gaps.push({ premise, reason: 'No live children-capable rc.9 owner at handler construction' }); return callback;
  }
  let scope = owners.get(owner);
  if (!scope) {
    scope = audit.scope('DOM handler owner'); owners.set(owner, scope);
    onCleanup(() => queueMicrotask(() => { state.ended++; scope.end(); }));
  }
  state.registrations.push({ premise, ownerId: owner.id ?? null });
  return scope.bind(callback);
}
