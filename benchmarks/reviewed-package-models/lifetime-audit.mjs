// Development observation under an explicit resource lifetime expectation.
// No package model, owner inference, accepted contract or cleanup is supplied.
export function lifetimeAudit(platform, { isAppFrame = () => false, onFeedback = () => {}, promiseContinuations = false } = {}) {
  const resources = new Set(), feedback = [], gaps = [], restores = [], timers = new Map(), observers = new WeakMap(), listeners = new WeakMap(), opaqueEventTargets = new WeakSet(), reported = new Set();
  let current = null, currentOrigin = null, nextScope = 0, nextResource = 0, stopped = false;
  let collectedResources = 0;
  function frames() {
    const prior = Error.stackTraceLimit; let stack;
    try { Error.stackTraceLimit = 100; stack = new Error().stack ?? ''; } finally { Error.stackTraceLimit = prior; }
    return stack.split('\n').flatMap(line => {
      const match = line.match(/(?:at .*?\()?((?:file:\/\/|https?:\/\/|\/).*?):(\d+):(\d+)\)?$/);
      return match ? [{ path: match[1], line: Number(match[2]), column: Number(match[3]) }] : [];
    });
  }
  function gap(reason, scope = current) { if (scope) gaps.push({ scope: scope.id, reason, frames: frames() }); }
  function reachable(resource) {
    if (resource.weakTargets?.some(ref => ref.deref() === undefined)) {
      resources.delete(resource); collectedResources++; return false;
    }
    return true;
  }
  function report(resource) {
    if (stopped || resource.reported || !resource.scope.ended || resource.scope.expectation !== 'stop') return;
    if (!reachable(resource) || resource.reachabilityOpen) return;
    resource.reported = true;
    const location = resource.frames.find(isAppFrame) ?? null, key = JSON.stringify([resource.scope.id, resource.kind, location ?? resource.frames]);
    if (reported.has(key)) return; reported.add(key);
    const item = { code: 'RESOURCE_OUTLIVES_DECLARED_SCOPE', severity: 'warning', category: 'lifetime-expectation',
      basis: 'live browser resource after declared scope end', certification: false,
      message: `${resource.kind} remains active after ${resource.scope.label} ended`,
      resource: resource.id, resourceKind: resource.kind, scope: resource.scope.id, ownerPath: [],
      frames: resource.frames, operationFrames: resource.operationFrames, location };
    feedback.push(item); onFeedback(item);
  }
  function add(kind, scope = current, endpoints = null) {
    if (!scope || stopped) return null;
    const operationFrames = frames();
    const reachabilityOpen = endpoints !== null && typeof platform.WeakRef !== 'function';
    if (reachabilityOpen) gap('native resource reachability is open without WeakRef', scope);
    const resource = { id: ++nextResource, kind, scope, frames: currentOrigin ?? operationFrames, operationFrames, reported: false,
      weakTargets: endpoints && !reachabilityOpen ? endpoints.map(target => new platform.WeakRef(target)) : null, reachabilityOpen };
    resources.add(resource); report(resource); return resource;
  }
  const drop = resource => { if (resource) resources.delete(resource); };
  function within(scope, fn, receiver, args, origin = null) {
    const previous = current, previousOrigin = currentOrigin; current = scope; currentOrigin = origin;
    try { return Reflect.apply(fn, receiver, args); } finally { current = previous; currentOrigin = previousOrigin; }
  }
  function patch(object, key, build) {
    const descriptor = Object.getOwnPropertyDescriptor(object, key);
    if (!descriptor || typeof descriptor.value !== 'function' || !descriptor.configurable) return;
    const wrapped = build(descriptor.value); Object.defineProperty(object, key, { ...descriptor, value: wrapped });
    restores.push(() => { if (Object.getOwnPropertyDescriptor(object, key)?.value === wrapped) Object.defineProperty(object, key, descriptor); });
  }
  // Opt-in experiment: native .then still creates and returns its own Promise.
  // Wrapping delivered callbacks adds no reaction or adoption job. Native await
  // continuations remain a separate source-instrumentation capability.
  if (promiseContinuations && platform.Promise?.prototype) patch(platform.Promise.prototype, 'then', native => function (yes, no) {
    const scope = current, origin = currentOrigin;
    if (!scope || stopped) return Reflect.apply(native, this, arguments);
    const bind = fn => typeof fn === 'function' ? function (...args) { return within(scope, fn, this, args, origin); } : fn;
    return Reflect.apply(native, this, [bind(yes), bind(no)]);
  });
  // Timeout and interval IDs share one native pool; either clear operation can
  // clear either kind. Native handles, receiver and callback arguments survive.
  for (const [schedule, cancel, kind, repeat, pool] of [
    ['setTimeout', 'clearTimeout', 'timeout', false, timers], ['setInterval', 'clearInterval', 'interval', true, timers],
    ['requestAnimationFrame', 'cancelAnimationFrame', 'animation-frame', false, new Map()],
    ['requestIdleCallback', 'cancelIdleCallback', 'idle-callback', false, new Map()],
  ]) {
    patch(platform, schedule, native => function (callback, ...args) {
      const scope = current;
      if (!scope || stopped || typeof callback !== 'function') { if (scope && typeof callback !== 'function') gap('string or non-function timer callback'); return Reflect.apply(native, this, [callback, ...args]); }
      let resource, handle;
      const invoke = function (...values) {
        if (!repeat) { drop(resource); pool.delete(handle); }
        return within(scope, callback, this, values, resource?.frames ?? null);
      };
      handle = Reflect.apply(native, this, [invoke, ...args]); resource = add(kind, scope); pool.set(handle, resource); return handle;
    });
    patch(platform, cancel, native => function (handle) {
      const result = Reflect.apply(native, this, arguments);
      if (!pool.has(handle) && handle != null && !(Number.isInteger(handle) && handle >= 0 && handle <= 0x7fffffff)) {
        // Native WebIDL can coerce a fractional, wrapped or otherwise opaque
        // handle to a tracked ID. Do not repeat that conversion or claim the
        // resource survived an unobserved cancellation.
        const scopes = new Set(); pool.forEach(resource => { if (resource) scopes.add(resource.scope); drop(resource); }); pool.clear();
        scopes.forEach(scope => gap('coerced timer cancellation is open', scope));
      } else { drop(pool.get(handle)); pool.delete(handle); }
      return result;
    });
  }
  // Observing a native instance is enough; constructor and callback identities
  // are unchanged. A callback's future resource creations are not propagated.
  for (const name of ['ResizeObserver', 'IntersectionObserver', 'MutationObserver']) {
    const prototype = platform[name]?.prototype; if (!prototype) continue;
    patch(prototype, 'observe', native => function (target) {
      const result = Reflect.apply(native, this, arguments);
      let record = observers.get(this); if (!record) observers.set(this, record = { targets: new WeakMap(), resources: new Set() });
      if (!record.targets.has(target)) { const resource = add(name, current, [this, target]); record.targets.set(target, resource); if (resource) record.resources.add(resource); } return result;
    });
    patch(prototype, 'unobserve', native => function (target) { const result = Reflect.apply(native, this, arguments); const record = observers.get(this), resource = record?.targets.get(target); drop(resource); record?.resources.delete(resource); record?.targets.delete(target); return result; });
    patch(prototype, 'disconnect', native => function () { const result = Reflect.apply(native, this, arguments); const record = observers.get(this); if (record) { record.resources.forEach(drop); record.resources.clear(); record.targets = new WeakMap(); } return result; });
  }
  const prototype = platform.EventTarget?.prototype;
  if (prototype) {
    function opaque(target, reason) {
      opaqueEventTargets.add(target); const scopes = new Set(); if (current) scopes.add(current);
      listeners.get(target)?.forEach(record => { if (record.resource) scopes.add(record.resource.scope); drop(record.resource); });
      listeners.delete(target); scopes.forEach(scope => gap(reason, scope));
    }
    // Forward native options reads exactly once, with the original receiver.
    // Once/signal removal is deliberately open: never manufacture a remaining
    // persistent listener from either path. Opaque type conversion is open too.
    function options(value) {
      const read = Object.create(null);
      const forwarded = value && (typeof value === 'object' || typeof value === 'function') ? new Proxy(value, {
        get(target, key) { const result = Reflect.get(target, key, target); read[key] = result; return result; },
      }) : value;
      return { forwarded, read, capture: () => !!(value && typeof value === 'object' || typeof value === 'function' ? read.capture : value) };
    }
    patch(prototype, 'addEventListener', native => function (type, callback, value) {
      if (typeof type !== 'string' || opaqueEventTargets.has(this) || callback === null || callback === undefined || stopped) {
        const result = Reflect.apply(native, this, arguments);
        if (!stopped && typeof type !== 'string') opaque(this, 'coerced event type is open');
        else if (!stopped && opaqueEventTargets.has(this)) gap('event target has an unresolved registration');
        return result;
      }
      const option = options(value), result = Reflect.apply(native, this, [type, callback, option.forwarded]);
      let records = listeners.get(this); if (!records) listeners.set(this, records = []);
      const capture = option.capture();
      if (records.some(record => record.type === type && record.callback === callback && record.capture === capture)) return result;
      const open = !!option.read.once || option.read.signal != null;
      if (open) gap('once or AbortSignal listener lifetime is open');
      records.push({ type, callback, capture, resource: open ? null : add('event-listener', current, [this]) }); return result;
    });
    patch(prototype, 'removeEventListener', native => function (type, callback, value) {
      if (typeof type !== 'string' || stopped) { const result = Reflect.apply(native, this, arguments); if (!stopped && typeof type !== 'string') opaque(this, 'coerced event type cancellation is open'); return result; }
      const option = options(value), result = Reflect.apply(native, this, [type, callback, option.forwarded]), capture = option.capture(), records = listeners.get(this);
      const index = records?.findIndex(record => record.type === type && record.callback === callback && record.capture === capture) ?? -1;
      if (index >= 0) { drop(records[index].resource); records.splice(index, 1); } return result;
    });
  }
  return {
    capture() {
      const scope = current, origin = currentOrigin;
      return scope ? { run(fn) { return within(scope, fn, undefined, [], origin); },
        bind(fn) { return typeof fn === 'function' ? function (...args) { return within(scope, fn, this, args, origin); } : fn; },
        member(target, key) {
          // Evaluate the receiver/property once before the source arguments.
          // Ordinary property access preserves primitive receivers and Proxy
          // getter semantics. Invocation retains the original member receiver.
          const fn = within(scope, () => target[key], undefined, [], origin);
          return typeof fn === 'function' ? function (...args) { return within(scope, fn, target, args, origin); } : fn;
        } } : null;
    },
    scope(label, expectation = 'stop') {
      if (!['stop', 'background'].includes(expectation)) throw new Error('Unknown explicit lifetime expectation');
      const scope = { id: ++nextScope, label, expectation, ended: false };
      return { run(fn) { return within(scope, fn, undefined, []); },
        bind(fn) { return function (...args) { return within(scope, fn, this, args); }; },
        end() { if (scope.ended) return; scope.ended = true; resources.forEach(resource => { if (resource.scope === scope) report(resource); }); } };
    },
    snapshot() { resources.forEach(reachable); return { active: [...resources].map(resource => ({ id: resource.id, kind: resource.kind, scope: resource.scope.id, ended: resource.scope.ended })), feedback, gaps, collectedResources }; },
    stop() { stopped = true; restores.reverse().forEach(restore => restore()); resources.clear(); timers.clear(); reported.clear(); current = null; currentOrigin = null; },
  };
}
