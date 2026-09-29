// ADR 0168 probe of `solid-js@2.0.0-rc.9`'s own createSignal and useContext, per build.
import * as J from 'solid-js';
import * as S from '@solidjs/signals';
const build = process.env.PROBE_BUILD;
const client = build.startsWith('client');
const out = [];
const note = (name, ok, detail = '') => out.push({ name, ok, detail });
if (client) {
  const { createRoot, createMemo, createSignal: sigSignal, flush } = S;
  function depsField() {
    let a, b;
    createRoot(() => { const [s] = sigSignal(1); const mb = createMemo(() => {}); const mc = createMemo(() => {}); const ma = createMemo(() => { s(); }); a = ma; b = mc; });
    const sym = Object.getOwnPropertySymbols(a)[0];
    const fields = Object.keys(a[sym]).filter(k => a[sym][k] != null && typeof a[sym][k] === 'object' && b[sym][k] == null);
    return { sym, fields };
  }
  const { sym, fields } = depsField();
  const tracks = fn => { let node; createRoot(() => { node = createMemo(() => { fn(); })[sym]; }); return fields.some(f => node[f] != null); };
  // Plain path, hydration off and on.
  for (const hydrating of [false, true]) {
    J.enableHydration?.();
    J.sharedConfig.hydrating = hydrating;
    let acc;
    const t = tracks(() => { [acc] = J.createSignal(5); });
    note(`createSignal(5), hydrating=${hydrating}: M gained a dependency`, t === false, String(t));
    note(`createSignal(5), hydrating=${hydrating}: reads back 5 with no gate`, (() => { let v; createRoot(() => { const [g] = J.createSignal(5, { ssrSource: 'client' }); v = g(); }); return v === 5; })());
  }
  // Function form: with hydration off the compute runs at the call; under hydration with ssrSource client
  // the archive's own compute reads the gate signal the call created (`hydrated()`, false at creation) and
  // returns before it reaches the caller's function, so the compute has not run when the call returns.
  let plain = 0, gated = 0;
  J.sharedConfig.hydrating = false;
  createRoot(() => { J.createSignal(() => { plain++; return 1; }, { ssrSource: 'client' }); });
  J.sharedConfig.hydrating = true;
  createRoot(() => { J.createSignal(() => { gated++; return 1; }, { ssrSource: 'client' }); });
  note('createSignal(fn, {ssrSource:"client"}): runs fn at the call when not hydrating, and not when hydrating (the archive read a gate it created)', plain === 1 && gated === 0, `plain=${plain} hydrating=${gated}`);
  J.sharedConfig.hydrating = false;
  // useContext
  const Ctx = J.createContext(7);
  note('useContext(ctx) in M: no dependency', tracks(() => J.useContext(Ctx)) === false);
  let v; createRoot(() => { v = J.useContext(Ctx); });
  note('useContext(ctx) returns the default', v === 7, String(v));
} else {
  let acc; J.createRoot(() => { [acc] = J.createSignal(5); });
  note('server createSignal(5) reads back 5', acc() === 5);
  let ctxv; const Ctx = J.createContext(7);
  J.createRoot(() => { ctxv = J.useContext(Ctx); });
  note('server useContext(ctx) returns the default', ctxv === 7, String(ctxv));
}
const bad = out.filter(r => !r.ok);
console.log(`[${build}] ${out.length - bad.length}/${out.length} as stated`);
for (const r of out) console.log(`  ${r.ok ? 'ok ' : 'BAD'} ${r.name}${r.detail ? '  (' + r.detail + ')' : ''}`);
process.exit(bad.length ? 1 : 0);
