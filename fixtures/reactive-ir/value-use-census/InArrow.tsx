// Inside `export const`.
import { createRoot, createSignal, untrack } from "solid-js";

export const Arrow = () => {
  const [c, set] = createSignal(1);
  { try { throw c; } catch {} }
  { try { void (1 + (() => { throw c; })()); } catch {} }
  { try { class A extends (c as any) {} void A; } catch {} }
  { void { [c as any]: 1 }; }
  { const m: any = {}; void m[c as any]; }
  { let h = c; void h; }
  { const b: { h?: unknown } = {}; b.h = c; }
  { const h = c || c; void h; }
  { class H { t = c; } void H; }
  { class H { constructor(public t: unknown) {} } void new H(c); }
  { const f = (h = c) => h; void f; }
  { void new Map([["k", c]]); }
  { const a = [c]; void a.at(0); }
  { const b = c.bind(null); void b; }
  { const t = (_: TemplateStringsArray, h: unknown) => h; void t`${c}`; }
  { ((h: unknown) => void h)(...([c] as [unknown])); }
  { const h = (() => c)(); void h; }
  { const w = { v: { v: c } }; void w; }
  { const f0 = (g: unknown) => g; const f1 = (g: unknown) => f0(g); void f1(c); }
  { const h = createRoot(() => c); void h; }
  { const h = untrack(() => c); void h; }
  { function* g() { yield c; } void g; }
  { (async () => { await c; })(); }
  { const useA = () => ({ read: c }); const hook = Math.random() >= 0 ? useA : useA; void hook; }
  { void Object.values({ c }); }
  { const o = { f() { return c; } }; void o; }
  { class K { f() { return c; } } void K; }
  { const o = { c }; const { c: h } = o; void h(); }
  { const H: any = c; void (<H />); }
  { try { throw set; } catch {} }
  { let g = set; void g; }
  { void Object.values({ set }); }
  return <p>x</p>;
};
