// ADR 0168 probe, on the exact rc.9 bytes of the audited archives (node_modules is a symlink to the
// provisioned install). Run the primitive as the compute of a tracking memo M, as ADR 0163's synthesized
// veto does, and observe whether M gained a dependency: whether the call made a tracked read of its own.
import * as S from '@solidjs/signals';
const { createRoot, createMemo, createSignal, getOwner, onCleanup, untrack, runWithOwner, flush } = S;
const build = process.env.PROBE_BUILD ?? 'prod';
// Find the dependency field the build's own layout uses: compare a memo that read a signal to one that read nothing.
function depsField() {
  let a, b;
  createRoot(() => {
    const [s] = createSignal(1);
    const mb = createMemo(() => {});
    const mc = createMemo(() => {});   // gives `a` a previous sibling too, so sibling links cancel out
    const ma = createMemo(() => { s(); });
    a = ma; b = mc;
  });
  const sym = Object.getOwnPropertySymbols(a)[0];
  const na = a[sym], nb = b[sym];
  const fields = Object.keys(na).filter(k => na[k] !== null && na[k] !== undefined && typeof na[k] === 'object' && (nb[k] === null || nb[k] === undefined));
  if (fields.length < 1 || fields.length > 2) throw new Error('cannot identify the dependency fields: ' + fields);
  return { sym, fields };
}
const { sym, fields } = depsField();
function tracks(fn) {
  let node;
  createRoot(() => {
    const m = createMemo(() => { fn(); });
    node = m[sym];
  });
  return fields.some(field => node[field] !== null && node[field] !== undefined);
}
const results = [];
function check(name, expectTracked, actual) {
  results.push({ name, expectTracked, actual, ok: expectTracked === actual });
}
let s, set;
createRoot(() => { [s, set] = createSignal(0); });
// control: a read by the memo itself is detected
check('control: s() in M', true, tracks(() => s()));
// the five signals-archive primitives, with the argument shapes each row states
check('getOwner()', false, tracks(() => getOwner()));
check('onCleanup(fn)', false, tracks(() => onCleanup(() => {})));
check('untrack(() => s())  [literal callable]', false, tracks(() => untrack(() => s())));
check('untrack(() => 1)', false, tracks(() => untrack(() => 1)));
check('runWithOwner(owner, () => s())  [literal callable]', false, tracks(() => runWithOwner(getOwner(), () => s())));
check('runWithOwner(null, () => 1)', false, tracks(() => runWithOwner(null, () => 1)));
check('createSignal(0)', false, tracks(() => createSignal(0)));
check('createSignal(0, { equals: false })', false, tracks(() => createSignal(0, { equals: false })));
check('createSignal(undefined)', false, tracks(() => createSignal(undefined)));
// function form: the memo the call creates computes eagerly, and reads inside it are its own
let inner = 0;
check('createSignal(() => { inner++; return s(); })  [function form; M itself]', false, tracks(() => createSignal(() => { inner++; return s(); })));
results.push({ name: '  ... and the function form did run its compute at the call', ok: inner === 1, actual: inner, expectTracked: 1 });
// the negative the argument scope exists for: a callable passed BY REFERENCE that is an accessor of a
// memo this very call created reads it, under untrack, and nothing about M's dependencies shows it.
let computes = 0;
const viaReference = () => {
  const lazy = createMemo(() => { computes++; return 1; }, { lazy: true });
  untrack(lazy);              // by reference: invokes the created memo's accessor
};
tracks(viaReference);
results.push({ name: 'untrack(createdMemoAccessor) [by reference] performed a read of a memo the call created', ok: computes === 1, actual: computes, expectTracked: 1 });
let computes2 = 0;
const viaLiteral = () => {
  const lazy = createMemo(() => { computes2++; return 1; }, { lazy: true });
  untrack(() => lazy());
};
tracks(viaLiteral);
results.push({ name: 'untrack(() => createdMemoAccessor())  [literal: the walk sees the call and refuses it]', ok: computes2 === 1, actual: computes2, expectTracked: 1 });
let computes3 = 0;
const viaRunWithOwner = () => {
  const lazy = createMemo(() => { computes3++; return 1; }, { lazy: true });
  runWithOwner(getOwner(), lazy);
};
tracks(viaRunWithOwner);
results.push({ name: 'runWithOwner(owner, createdMemoAccessor) [by reference] performed a read', ok: computes3 === 1, actual: computes3, expectTracked: 1 });
// createSignal(createdMemoAccessor): the function form invokes it at creation
let computes4 = 0;
const viaCreateSignal = () => {
  const lazy = createMemo(() => { computes4++; return 1; }, { lazy: true });
  createSignal(lazy);
};
tracks(viaCreateSignal);
results.push({ name: 'createSignal(createdMemoAccessor) [by reference, function form] performed a read', ok: computes4 === 1, actual: computes4, expectTracked: 1 });
const bad = results.filter(r => !r.ok);
console.log(`[${build}] ${results.length - bad.length}/${results.length} as stated`);
for (const r of results) console.log(`  ${r.ok ? 'ok ' : 'BAD'} ${r.name}  -> ${typeof r.expectTracked === 'boolean' ? 'tracked=' + r.actual : 'count=' + r.actual}`);
process.exit(bad.length ? 1 : 0);
