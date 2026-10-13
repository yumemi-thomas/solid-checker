// Authored after the shared collector was sealed. Preserve executed consumers.
import { relative, resolve } from 'node:path';
import { read } from './catalog.mjs';
const foreign = read(resolve('rust/target/cross-package-roots/run.json'));
const cases = [];
const prelude = `import { createSignal, createRoot, createTrackedEffect, getOwner, onCleanup, runWithOwner, flush, untrack } from 'solid-js';
import { render } from '@solidjs/web'; const h = (globalThis as any).__experiment;
h.values.calls = 0; h.values.cleanups = 0; h.values.caught = [];`;
const styles = [
  { id: 'lodash-caught', package: 'lodash', imports: `import lodash from 'lodash';`,
    setup: `const dispatch = (fn: () => number) => { const result = lodash.attempt(fn); if (result instanceof Error) h.values.caught.push(result.message); return result; };` },
  { id: 'rxjs-error-channel', package: 'rxjs', imports: `import { of, EMPTY, map, asyncScheduler, observeOn } from 'rxjs';`,
    setup: `const dispatch = (fn: () => number) => of(1).pipe(map(() => fn())).subscribe({ error: error => h.values.caught.push(error.message) });` },
  { id: 'neverthrow-map', package: 'neverthrow', imports: `import { ok, ResultAsync } from 'neverthrow';`,
    setup: `const dispatch = (fn: () => number) => ok(1).map(() => fn());` },
  { id: 'zod-transform', package: 'zod', imports: `import { z } from 'zod';`,
    setup: `const dispatch = (fn: () => number) => z.number().transform(() => fn()).parse(1);` },
  { id: 'valibot-transform', package: 'valibot', imports: `import * as v from 'valibot';`,
    setup: `const dispatch = (fn: () => number) => v.parse(v.pipe(v.number(), v.transform(() => fn())), 1);` },
  { id: 'query-cache-notify', package: '@tanstack/query-core', imports: `import { QueryClient, QueryCache } from '@tanstack/query-core';`,
    setup: `const dispatch = (fn: () => number) => { const cache = new QueryCache(), client = new QueryClient({ queryCache: cache }); const stop = cache.subscribe(() => fn());
      try { cache.build(client, { queryKey: ['probe'], queryFn: async () => 1 }); } finally { stop(); } return 1; };` },
  { id: 'floating-initial-update', package: '@floating-ui/dom', imports: `import { autoUpdate } from '@floating-ui/dom';`,
    setup: `const dispatch = (fn: () => number) => { const reference = document.createElement('button'), floating = document.createElement('div');
      const clear = autoUpdate(reference, floating, () => { fn(); }, { ancestorScroll: false, ancestorResize: false, elementResize: false, layoutShift: false }); clear(); return 1; };` },
];
function install(name) { const row = foreign.results.find(row => row.package === name); if (!row) throw new Error(name);
  return relative(resolve('rust/target/app-import-metric/apps'), row.retainedArtifacts.projectDir); }
function add(id, style, role, body, flow, expected = {}) {
  cases.push({ id: 'context-' + id + '-' + role, package: style.package, app: install(style.package),
    source: `${prelude}\n${style.imports}\nfunction App() { ${style.setup}\n${body}\nreturn <p>callback probe</p>; }
h.attempt('mount', () => h.dispose = render(() => <App />, document.getElementById('root')!));`,
    flow, provenance: { pair: id, role, expectedIssue: role === 'target', callbackExpected: true, ...expected } });
}
function observe(desired, { disposeFirst = false } = {}) {
  return async page => {
    if (disposeFirst) await page.evaluate(() => { const h = globalThis.__experiment; h.dispose(); h.disposals++; });
    await page.evaluate(() => { const h = globalThis.__experiment; h.attempt('invoke', () => h.run?.()); });
    await page.waitForTimeout(60);
    await page.evaluate(disposeFirst => { const h = globalThis.__experiment; h.attempt('flush', () => h.flush?.());
      if (!disposeFirst) { h.dispose(); h.disposals++; } h.extraDispose?.(); }, disposeFirst);
    await page.waitForTimeout(20);
    await page.evaluate(desired => { const h = globalThis.__experiment; h.values.behavior = { desired,
      actual: JSON.stringify({ calls: h.values.calls, value: h.readValue?.() ?? null, cleanups: h.values.cleanups }) }; }, desired);
  };
}
const shape = (calls, value = null, cleanups = 0) => JSON.stringify({calls, value, cleanups});
for (const style of styles) {
  const write = `const [value, set] = createSignal(1); h.flush = flush; h.readValue = () => untrack(value);
    const callback = () => { h.values.calls++; set(2); return 1; }; const invoke = () => dispatch(callback);`;
  for (const bad of [true, false]) add(style.id + '-write', style, bad ? 'target' : 'control',
    write + (bad ? `h.attempt('setup-call', invoke);` : `h.run = invoke;`), observe(shape(1,2)),
    { family: 'foreign-synchronous-owned-write', rules: [], codes: ['REACTIVE_WRITE_IN_OWNED_SCOPE'] });
  const cleanup = `const callback = () => { h.values.calls++; onCleanup(() => h.values.cleanups++); return 1; };`;
  for (const bad of [true, false]) add(style.id + '-leaf-cleanup', style, bad ? 'target' : 'control',
    cleanup + (bad ? `createTrackedEffect(() => { h.attempt('leaf-call', () => dispatch(callback)); });` : `dispatch(callback);`),
    observe(shape(1,null,1)), { family: 'foreign-leaf-cleanup', rules: [], codes: ['CLEANUP_IN_FORBIDDEN_SCOPE'] });
}
for (const style of styles.filter(style => ['rxjs','valibot','neverthrow','@tanstack/query-core'].includes(style.package))) {
  for (const bad of [true, false]) add(style.id + '-delayed-owner', style, bad ? 'target' : 'control',
    `const owner = getOwner(); const callback = () => { h.values.calls++; onCleanup(() => h.values.cleanups++); return 1; };
      h.run = () => queueMicrotask(() => h.attempt('delivered', () => ${bad ? 'dispatch(callback)' : 'runWithOwner(owner, () => dispatch(callback))'}));`,
    observe(shape(1,null,1)), { family: 'delayed-callback-cleanup', rules: [], codes: ['NO_OWNER_CLEANUP'] });
}
for (const style of styles.filter(style => ['lodash','rxjs'].includes(style.package))) {
  for (const bad of [true,false]) add(style.id + '-two-callers', style, bad ? 'target' : 'control',
    `const [,set] = createSignal(1); const shared = () => { h.values.calls++; set(2); return 1; };
      function first() { dispatch(shared); } function second() { dispatch(shared); }
      ${bad ? "h.attempt('first', first); h.attempt('second', second);" : 'h.run = () => { first(); second(); };'}`,
    observe(shape(2)), { family: 'distinct-caller-attribution', rules: [], codes: ['REACTIVE_WRITE_IN_OWNED_SCOPE'], expectedFeedbackSites: bad ? 2 : 0 });
}
const rxjs = styles.find(style=>style.package==='rxjs'), lodash=styles.find(style=>style.package==='lodash'), zod=styles.find(style=>style.package==='zod'), neverthrow=styles.find(style=>style.package==='neverthrow');
for (const [style,id,body] of [
  [lodash,'trailing-write', `const [value,set]=createSignal(1); h.flush=flush; h.readValue=()=>untrack(value); lodash.debounce(() => { h.values.calls++; set(2); },5)();`],
  [rxjs,'scheduled-write', `const [value,set]=createSignal(1); h.flush=flush; h.readValue=()=>untrack(value); of(1).pipe(observeOn(asyncScheduler),map(() => { h.values.calls++; set(2); return 1; })).subscribe();`],
  [zod,'async-transform-write', `const [value,set]=createSignal(1); h.flush=flush; h.readValue=()=>untrack(value); z.number().transform(async () => { await Promise.resolve(); h.values.calls++; set(2); return 1; }).parseAsync(1);`],
  [neverthrow,'async-result-write', `const [value,set]=createSignal(1); h.flush=flush; h.readValue=()=>untrack(value); ResultAsync.fromSafePromise(Promise.resolve(1)).map(() => { h.values.calls++; set(2); return 1; });`],
]) add(id,style,'control',body,observe(shape(1,2)),{family:'deferred-valid-write',rules:[],codes:[]});
add('empty-stream',rxjs,'control',`EMPTY.pipe(map(() => { h.values.calls++; onCleanup(() => {}); return 1; })).subscribe();`,
  observe(shape(0)),{family:'callback-not-executed',callbackExpected:false,rules:[],codes:[]});
for(const bad of [true,false]) add('disposed-owner',neverthrow,bad?'target':'control',
  `const owner=getOwner(); const callback=()=>{h.values.calls++; onCleanup(()=>h.values.cleanups++); return 1;};
    h.run=()=>${bad ? 'runWithOwner(owner,()=>dispatch(callback))' : 'h.extraDispose=createRoot(dispose=>{ dispatch(callback); return dispose; })'};`,
  observe(shape(1,null,1),{disposeFirst:true}),{family:'disposed-callback-owner',rules:[],codes:['RUN_WITH_DISPOSED_OWNER']});
for(const [id,style,body,code] of [
  ['rxjs-wrong-callback',rxjs,`of(1).pipe(map((value:string)=>value));`,2345],
  ['query-wrong-key',styles.find(style=>style.package==='@tanstack/query-core'),`new QueryClient().setQueryData(42,1);`,2345],
  ['zod-missing-member',zod,`z.number().missingMember();`,2339],
]) add(id,style,'control',body,observe(shape(0)),{family:'type-exclusion',callbackExpected:false,rules:[],codes:[],expectedTypingCode:code});
export default cases;
