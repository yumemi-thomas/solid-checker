import assert from 'node:assert/strict';
import test from 'node:test';
import { lifetimeAudit } from './lifetime-audit.mjs';
function fixture() {
  let next = 0; const pending = new Map();
  const platform = {
    WeakRef,
    setTimeout(callback, delay, ...args) { const id = ++next; pending.set(id, { callback, args, repeat: false }); return id; },
    setInterval(callback, delay, ...args) { const id = ++next; pending.set(id, { callback, args, repeat: true }); return id; },
    clearTimeout(id) { pending.delete(id); }, clearInterval(id) { pending.delete(id); },
    requestAnimationFrame(callback) { return this.setTimeout(callback, 0, 42); }, cancelAnimationFrame(id) { this.clearTimeout(id); },
    requestIdleCallback(callback) { return this.setTimeout(callback, 0, { didTimeout: false }); }, cancelIdleCallback(id) { this.clearTimeout(id); },
  };
  class Target extends EventTarget {
    addEventListener(...args) { return super.addEventListener(...args); }
    removeEventListener(...args) { return super.removeEventListener(...args); }
  }
  class Observer {
    targets = new Set();
    observe(target) { this.targets.add(target); }
    unobserve(target) { this.targets.delete(target); }
    disconnect() { this.targets.clear(); }
  }
  platform.EventTarget = Target; platform.ResizeObserver = Observer;
  return { platform, pending, fire(id, receiver = platform) { const item = pending.get(id); if (!item.repeat) pending.delete(id); return Reflect.apply(item.callback, receiver, item.args); } };
}
test('a shared timeout/interval pool honors cross-kind cancellation and completed callbacks', () => {
  const f = fixture(), audit = lifetimeAudit(f.platform), scope = audit.scope('component');
  const [first, second, third] = scope.run(() => [f.platform.setTimeout(() => {}, 10), f.platform.setInterval(() => {}, 10), f.platform.setTimeout(() => {}, 10)]);
  f.platform.clearInterval(first); f.platform.clearTimeout(second); f.fire(third); scope.end();
  assert.deepEqual(audit.snapshot().feedback, []); assert.deepEqual(audit.snapshot().active, []); audit.stop();
});
test('resource callbacks preserve receiver, arguments and return while propagating an explicit scope', () => {
  const f = fixture(), audit = lifetimeAudit(f.platform), scope = audit.scope('component'), receiver = {};
  const id = scope.run(() => f.platform.setTimeout(function (value) { assert.equal(this, receiver); f.platform.setTimeout(() => {}, 10); return value; }, 10, 7));
  assert.equal(f.fire(id, receiver), 7); scope.end();
  assert.equal(audit.snapshot().feedback.length, 1); assert.equal(audit.snapshot().feedback[0].resourceKind, 'timeout'); audit.stop();
});
test('a thrown one-shot callback keeps error identity and releases its resource', () => {
  const f = fixture(), audit = lifetimeAudit(f.platform), scope = audit.scope('component'), error = new Error('original');
  const id = scope.run(() => f.platform.setTimeout(() => { throw error; }, 10));
  assert.throws(() => f.fire(id), value => value === error); scope.end(); assert.deepEqual(audit.snapshot().feedback, []); audit.stop();
});
test('observer targets are cleared precisely and a restarted disposed instance is reported', () => {
  const f = fixture(), audit = lifetimeAudit(f.platform), scope = audit.scope('component'), observer = new f.platform.ResizeObserver(), first = {}, second = {};
  scope.run(() => { observer.observe(first); observer.observe(second); observer.observe(first); }); observer.unobserve(first); observer.disconnect(); scope.end();
  assert.equal(audit.snapshot().feedback.length, 0); scope.run(() => observer.observe(second));
  assert.equal(audit.snapshot().feedback.length, 1); observer.disconnect(); audit.stop();
});
test('persistent listeners preserve callback identity, this, capture matching and duplicate semantics', () => {
  const f = fixture(), audit = lifetimeAudit(f.platform), scope = audit.scope('component'), target = new f.platform.EventTarget(); let calls = 0;
  const callback = function () { assert.equal(this, target); calls++; };
  scope.run(() => { target.addEventListener('test', callback); target.addEventListener('test', callback, false); });
  target.dispatchEvent(new Event('test')); assert.equal(calls, 1); assert.equal(audit.snapshot().active.length, 1);
  target.removeEventListener('test', callback, true); assert.equal(audit.snapshot().active.length, 1);
  target.removeEventListener('test', callback, false); scope.end(); assert.equal(audit.snapshot().feedback.length, 0); audit.stop();
});
test('options getters use the original receiver once per native read; once/signal remain open', () => {
  const f = fixture(), audit = lifetimeAudit(f.platform), scope = audit.scope('component'), target = new f.platform.EventTarget();
  const counts = {}, options = {};
  for (const [key, value] of Object.entries({ capture: false, once: true, passive: false, signal: undefined })) Object.defineProperty(options, key, { get() { assert.equal(this, options); counts[key] = (counts[key] ?? 0) + 1; return value; } });
  let calls = 0; scope.run(() => target.addEventListener('test', () => calls++, options));
  target.dispatchEvent(new Event('test')); target.dispatchEvent(new Event('test')); scope.end(); assert.equal(calls, 1);
  assert(Object.values(counts).every(count => count === 1)); assert.equal(audit.snapshot().gaps.length, 1); assert.equal(audit.snapshot().feedback.length, 0); audit.stop();
});
test('a listener first registered outside the scope is not reassigned by a duplicate', () => {
  const f = fixture(), audit = lifetimeAudit(f.platform), scope = audit.scope('component'), target = new f.platform.EventTarget(), callback = () => {};
  target.addEventListener('test', callback); scope.run(() => target.addEventListener('test', callback)); scope.end();
  assert.deepEqual(audit.snapshot().feedback, []); target.removeEventListener('test', callback); audit.stop();
});
test('bound callbacks restore nesting; background expectation and monitor shutdown remain quiet', () => {
  const f = fixture(), original = f.platform.setTimeout, audit = lifetimeAudit(f.platform), component = audit.scope('component'), background = audit.scope('background', 'background');
  const fn = background.bind(function (value) { assert.equal(this, f.platform); f.platform.setTimeout(() => {}, 10); return value; });
  assert.equal(component.run(() => fn.call(f.platform, 3)), 3); component.end(); background.end(); assert.deepEqual(audit.snapshot().feedback, []);
  audit.stop(); assert.equal(f.platform.setTimeout, original); assert.deepEqual(audit.snapshot().active, []);
});
test('a recurring callback chain retains its origin and emits one warning for the same lifetime issue', () => {
  const f = fixture(), audit = lifetimeAudit(f.platform), scope = audit.scope('component'); let id;
  function loop() { id = f.platform.setTimeout(loop, 10); }
  scope.run(() => { id = f.platform.setTimeout(loop, 10); }); scope.end();
  const origin = audit.snapshot().feedback[0].frames;
  for (let i = 0; i < 4; i++) f.fire(id);
  assert.equal(audit.snapshot().feedback.length, 1); assert.deepEqual(audit.snapshot().feedback[0].frames, origin);
  assert.equal(audit.snapshot().active.length, 1); f.platform.clearTimeout(id); audit.stop();
});
test('opaque cancellation never turns a missing registry key into proof that the resource survives', () => {
  const f = fixture(), audit = lifetimeAudit(f.platform), scope = audit.scope('component');
  const handle = scope.run(() => f.platform.setTimeout(() => {}, 10)); f.platform.clearTimeout(handle + 0.75);
  const target = new f.platform.EventTarget(), callback = () => {};
  scope.run(() => target.addEventListener('test', callback)); target.removeEventListener({ toString: () => 'test' }, callback);
  scope.end(); assert.equal(audit.snapshot().feedback.length, 0); assert.equal(audit.snapshot().gaps.length, 2); audit.stop();
});
test('missing weak reachability produces a gap instead of a listener lifetime warning', () => {
  const f = fixture(); delete f.platform.WeakRef;
  const audit = lifetimeAudit(f.platform), scope = audit.scope('component'), target = new f.platform.EventTarget();
  scope.run(() => target.addEventListener('test', () => {})); scope.end();
  assert.equal(audit.snapshot().feedback.length, 0); assert.equal(audit.snapshot().gaps[0].reason, 'native resource reachability is open without WeakRef'); audit.stop();
});
