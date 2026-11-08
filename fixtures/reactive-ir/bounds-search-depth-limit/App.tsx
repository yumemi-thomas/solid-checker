// ADR 0750: each walk answers within its limit and is undecided past it (`search-depth-limit`).
import { createEffect, createMemo, createSignal, flush } from "solid-js";

// effect_phase MAX_LAYERS 12: twelve helpers from the apply are proven, thirteen are undecided.
function fa12(): void { flush(); }
function fa11(): void { fa12(); }
function fa10(): void { fa11(); }
function fa9(): void { fa10(); }
function fa8(): void { fa9(); }
function fa7(): void { fa8(); }
function fa6(): void { fa7(); }
function fa5(): void { fa6(); }
function fa4(): void { fa5(); }
function fa3(): void { fa4(); }
function fa2(): void { fa3(); }
function fa1(): void { fa2(); }
function fb13(): void { flush(); }
function fb12(): void { fb13(); }
function fb11(): void { fb12(); }
function fb10(): void { fb11(); }
function fb9(): void { fb10(); }
function fb8(): void { fb9(); }
function fb7(): void { fb8(); }
function fb6(): void { fb7(); }
function fb5(): void { fb6(); }
function fb4(): void { fb5(); }
function fb3(): void { fb4(); }
function fb2(): void { fb3(); }
function fb1(): void { fb2(); }
function ma12(): void { createMemo(() => 1); }
function ma11(): void { ma12(); }
function ma10(): void { ma11(); }
function ma9(): void { ma10(); }
function ma8(): void { ma9(); }
function ma7(): void { ma8(); }
function ma6(): void { ma7(); }
function ma5(): void { ma6(); }
function ma4(): void { ma5(); }
function ma3(): void { ma4(); }
function ma2(): void { ma3(); }
function ma1(): void { ma2(); }
function mb13(): void { createMemo(() => 1); }
function mb12(): void { mb13(); }
function mb11(): void { mb12(); }
function mb10(): void { mb11(); }
function mb9(): void { mb10(); }
function mb8(): void { mb9(); }
function mb7(): void { mb8(); }
function mb6(): void { mb7(); }
function mb5(): void { mb6(); }
function mb4(): void { mb5(); }
function mb3(): void { mb4(); }
function mb2(): void { mb3(); }
function mb1(): void { mb2(); }
export function Phase() {
  const [a] = createSignal(1);
  createEffect(a, () => { fa1(); });
  createEffect(a, () => { fb1(); });
  createEffect(a, () => { ma1(); });
  createEffect(a, () => { mb1(); });
  return <div>{a()}</div>;
}

// after_await MAX_DEPTH 6, native_promise MAX_DEPTH 6: a read in a handler past ten `then`s.
const [r] = createSignal(1);
export function Chain() {
  const m = createMemo(async () => Promise.resolve(1).then((x) => x).then((x) => x).then((x) => x).then((x) => x).then((x) => x).then((x) => x).then((x) => x).then((x) => x).then((x) => x).then(() => r()));
  return <div>{m()}</div>;
}

// static_rules region_calls_handed, three regions: a handed accessor called four helpers down after an await.
function k4(g: () => number) { return g(); }
function k3(g: () => number) { return k4(g); }
function k2(g: () => number) { return k3(g); }
function k1(g: () => number) { return k2(g); }
function Kid(props: { f: () => number }) {
  const m = createMemo(async () => { await Promise.resolve(); return k1(props.f); });
  return <div>{m()}</div>;
}
export function Handed() { return <Kid f={() => 1} />; }
