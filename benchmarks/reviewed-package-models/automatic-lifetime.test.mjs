import assert from 'node:assert/strict';
import test from 'node:test';
import { resolve } from 'node:path';
import { transformLifetime } from './automatic-lifetime-transform.mjs';
import { background, ownerEvent, state } from './automatic-lifetime-runtime.mjs';
import { lifetimeAudit } from './lifetime-audit.mjs';
const path = resolve('rust/target/app-import-metric/apps/helge-dev/src/__automatic_lifetime_test.tsx');
const load = value => import('data:text/javascript;base64,' + Buffer.from(value.replaceAll('/@fs/', 'file:///')).toString('base64'));
test('only renderer-owned intrinsic event declarations supply an automatic binding', () => {
  const code = `import {} from 'solid-js'; const handle = () => {}; function Component(props: { onClick: () => void }) { return null; } const a = <button onClick={handle}/>; const b = <Component onClick={handle}/>;`;
  const result = transformLifetime(code, path); assert.equal(result.metadata.length, 1); assert.equal(result.metadata[0].tag, 'button');
  assert(result.metadata[0].declarations.every(declaration => declaration.path.endsWith('/@solidjs/web/types/jsx.d.ts')));
  assert(result.code.includes('onClick={__resourceLifetimeEvent(handle,')); assert(result.code.includes('<Component onClick={handle}/>'));
});
test('the exact compiler-emitted awaiter wraps all three resumptions; native profile keeps async', () => {
  const code = `import {} from 'solid-js'; async function click() { await Promise.resolve(); return 7; } const x = <button onClick={click}/>;`;
  const native = transformLifetime(code, path), lowered = transformLifetime(code, path, { asyncContext: true });
  assert.equal(native.helper, null); assert(native.code.includes('async function click'));
  assert.equal(lowered.helper.resumptions, 3); assert(lowered.code.includes('__resourceResume.run(() => generator.next(value))'));
  assert(lowered.code.includes('__resourceResume.run(() => generator["throw"](value))')); assert(lowered.map.sourcesContent.includes(code));
});
test('async generator and for-await are explicit lowering gaps', () => {
  for (const code of [`export async function* stream() { yield 1; }`, `export async function read(values) { for await (const value of values) {} }`]) {
    const result = transformLifetime(code, path, { asyncContext: true }); assert.equal(result.helper, null); assert.equal(result.gaps.length, 1); assert(result.code.includes('async'));
  }
});
test('owner binding preserves callback receiver, arguments, return and cleanup without restoring Solid ownership', async () => {
  const previous = globalThis.__resourceAudit, cleanups = [], owner = { _config: 0, _root: true, id: 1 }, receiver = {};
  const handles = new Set(); const platform = { setTimeout(fn) { const handle = handles.size + 1; handles.add(handle); return handle; }, clearTimeout(handle) { handles.delete(handle); } };
  const audit = globalThis.__resourceAudit = lifetimeAudit(platform);
  try {
    const callback = function (arg) { assert.equal(this, receiver); platform.setTimeout(() => {}, 100); return arg; };
    const bind = () => ownerEvent(callback, () => owner, fn => cleanups.push(fn), 'premise');
    const wrapped = bind(); bind(); assert.equal(cleanups.length, 1); assert.equal(wrapped.call(receiver, 7), 7);
    cleanups[0](); await Promise.resolve(); assert.equal(audit.snapshot().feedback.length, 1);
  } finally { audit.stop(); globalThis.__resourceAudit = previous; }
});
test('background identity and forbidden or unknown owners are preserved', () => {
  const previous = globalThis.__resourceAudit; globalThis.__resourceAudit = { scope() { throw new Error('Must stay open'); } };
  const fn = background(() => 3), forbidden = { _config: 16, _flags: 0 }, before = state.gaps.length;
  try {
    assert.equal(ownerEvent(fn, () => { throw new Error('Background should bypass owner lookup'); }, () => {}, 'background'), fn);
    const ordinary = () => 4;
    for (const owner of [null, {}, forbidden, { _config: 0, _flags: 64 }]) assert.equal(ownerEvent(ordinary, () => owner, () => { throw new Error('No cleanup allowed'); }, 'unknown'), ordinary);
    assert.equal(state.gaps.length - before, 4);
  } finally { globalThis.__resourceAudit = previous; }
});
test('capturing an audit context does not retain it across unrelated asynchronous work', async () => {
  const platform = { setTimeout() { return 1; }, clearTimeout() {} }, audit = lifetimeAudit(platform), scope = audit.scope('event');
  assert.equal(audit.capture(), null); const captured = scope.run(() => audit.capture()); assert.equal(audit.capture(), null);
  await Promise.resolve(); assert.equal(audit.capture(), null); captured.run(() => platform.setTimeout(() => {}, 10)); scope.end();
  assert.equal(audit.snapshot().feedback.length, 1); audit.stop();
});
test('native continuation instrumentation preserves receivers, argument order, return values and Promise adoption', async () => {
  const code = `export async function probe(log) { const receiver = { get method() { log.push('getter'); return function(v) { log.push(this === receiver ? 'receiver' : 'wrong'); return v; }; } }; await Promise.resolve(); const value = receiver.method((log.push('argument'), 7)); return value; }
export async function order(log) { log.push('start'); await new (class extends Promise { then(yes, no) { log.push('then'); return super.then(yes, no); } })(resolve => resolve(1)); log.push('resumed'); }`;
  const instrumented = transformLifetime(code, path, { nativeContext: true });
  assert.equal(instrumented.helper, null); assert(instrumented.code.includes('async function order'));
  assert(instrumented.gaps.every(gap => gap.reason.startsWith('Ordinary nested function')));
  const original = await load(code), changed = await load(instrumented.code), prior = globalThis.__resourceAudit;
  let runs = 0; globalThis.__resourceAudit = { capture: () => ({ run(fn) { runs++; return fn(); }, bind(fn) { return fn; } }) };
  try {
    const a = [], b = []; assert.equal(await original.probe(a), 7); assert.equal(await changed.probe(b), 7); assert.deepEqual(b, a);
    const run = async module => { const log = []; const pending = module.order(log); log.push('after-call'); queueMicrotask(() => log.push('microtask')); Promise.resolve().then(() => log.push('then-job')); await pending; return log; };
    assert.deepEqual(await run(changed), await run(original)); assert(runs > 0);
  } finally { globalThis.__resourceAudit = prior; }
});
test('native instrumentation states operand, optional-chain, direct-eval and deferred callback gaps', () => {
  const code = `export async function argument(object) { new object.Type(await Promise.resolve()); }
export async function optional(object) { await Promise.resolve(); object?.method().next(); }
export async function direct() { const local = 7; await Promise.resolve(); return eval('local'); }
export async function deferred(callback) { await Promise.resolve(); Promise.resolve().then(() => callback()); }
export async function empty() {}`;
  const result = transformLifetime(code, path, { nativeContext: true });
  assert(result.gaps.some(gap => gap.reason.startsWith('Call with awaited'))); assert(result.gaps.some(gap => gap.reason.startsWith('Optional call')));
  assert(result.gaps.some(gap => gap.reason.startsWith('Direct eval'))); assert(result.code.includes("return eval('local')"));
  assert(result.code.includes('() => callback()')); assert.equal(result.continuations.length, 4);
});
test('binding a value callee retains argument awaits and catches resources from a returned function', async () => {
  const code = `export async function probe(factory, argument) { factory(await argument())(); }`;
  const transformed = transformLifetime(code, path, { nativeContext: true }); assert.equal(transformed.gaps.length, 0);
  const module = await load(transformed.code);
  const platform = { setTimeout() { return 1; }, clearTimeout() {} }, audit = lifetimeAudit(platform), scope = audit.scope('event'), prior = globalThis.__resourceAudit;
  globalThis.__resourceAudit = audit;
  const order = [];
  try {
    const pending = scope.run(() => module.probe(value => { order.push('factory:' + value); return () => { order.push('call'); platform.setTimeout(() => {}, 100); }; }, async () => { order.push('argument'); await Promise.resolve(); return 7; }));
    await pending; scope.end(); assert.deepEqual(order, ['argument', 'factory:7', 'call']); assert.equal(audit.snapshot().feedback.length, 1);
  } finally { audit.stop(); globalThis.__resourceAudit = prior; }
});
test('opt-in Promise delivery retains results, rejection identity and native ordering while propagating lifetime', async () => {
  const platform = { Promise, setTimeout() { return 1; }, clearTimeout() {} }, audit = lifetimeAudit(platform, { promiseContinuations: true }), scope = audit.scope('event');
  const error = new Error('expected'), log = [];
  try {
    const result = scope.run(() => Promise.resolve(7).then(function (value) { assert.equal(this, undefined); log.push('callback'); platform.setTimeout(() => {}, 10); return value + 1; }));
    log.push('after-register'); queueMicrotask(() => log.push('microtask')); assert.equal(await result, 8);
    assert.deepEqual(log, ['after-register', 'callback', 'microtask']); scope.end(); assert.equal(audit.snapshot().feedback.length, 1);
    await assert.rejects(scope.run(() => Promise.resolve().then(() => { throw error; })), value => value === error);
  } finally { audit.stop(); }
});
test('plain TypeScript helpers preserve native awaits and generic assertions', () => {
  const code = `export async function helper<T>(value: T) { await Promise.resolve(); return (<T>value); }`;
  const result = transformLifetime(code, path.replace('.tsx', '.ts'), { nativeContext: true });
  assert.equal(result.continuations.length, 1); assert(result.code.includes('await')); assert.equal(result.gaps.length, 0);
});
test('a consumer binding named globalThis does not shadow the imported audit capture', async () => {
  const code = `export async function probe(globalThis, schedule) { await Promise.resolve(); schedule(globalThis); }`;
  const module = await load(transformLifetime(code, path, { nativeContext: true }).code), prior = globalThis.__resourceAudit;
  const platform = { setTimeout() { return 1; }, clearTimeout() {} }, audit = globalThis.__resourceAudit = lifetimeAudit(platform), scope = audit.scope('event');
  try { await scope.run(() => module.probe(7, value => { assert.equal(value, 7); platform.setTimeout(() => {}, 10); })); scope.end(); assert.equal(audit.snapshot().feedback.length, 1); }
  finally { audit.stop(); globalThis.__resourceAudit = prior; }
});
test('awaited member calls preserve receiver, property conversion, getter order and thrown identity', async () => {
  const code = `export async function probe(receiver, key, log) { return receiver[await key](await (log.push('argument'), Promise.resolve(7))); }
export async function primitive() { return 'abc'.charAt(await Promise.resolve(1)); }`;
  const module = await load(transformLifetime(code, path, { nativeContext: true }).code), original = await load(code), prior = globalThis.__resourceAudit;
  const platform = { setTimeout() { return 1; }, clearTimeout() {} }, audit = globalThis.__resourceAudit = lifetimeAudit(platform), scope = audit.scope('event');
  const run = async (module, scoped) => { const log = [], receiver = { get method() { log.push('getter'); return function (value) { assert.equal(this, receiver); log.push('call'); platform.setTimeout(() => {}, 10); return value + 1; }; } };
    const key = { [Symbol.toPrimitive]() { log.push('key'); return 'method'; } }; const invoke = () => module.probe(receiver, key, log);
    assert.equal(await (scoped ? scope.run(invoke) : invoke()), 8); return log; };
  try {
    assert.deepEqual(await run(module, true), await run(original, false)); assert.equal(await scope.run(() => module.primitive()), 'b');
    const error = new Error('getter failure'), receiver = { get method() { throw error; } };
    await assert.rejects(scope.run(() => module.probe(receiver, 'method', [])), value => value === error);
    scope.end(); assert.equal(audit.snapshot().feedback.length, 1);
  } finally { audit.stop(); globalThis.__resourceAudit = prior; }
});
