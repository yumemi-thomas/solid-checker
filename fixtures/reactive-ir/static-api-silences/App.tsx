// ADR 0989: the static API's calls end. Each family's call is a site (`effect-fn`: a call of an effect primitive, `sync-option`: a call of a primitive that
// takes `sync`) and ends in a row or in a silence that names why: SC7001 `missing-effect-function`, SC7002 `sync-computation-received-async`. Chrome rc.21
// (probed, `patterns/syncopt/`): the runtime reads the LAST `sync` property of an options literal, and `createTrackedEffect`'s `sync` is inert.
import * as S from "solid-js";
import { createEffect, createMemo, createOptimistic, createRenderEffect, createSignal, createTrackedEffect } from "solid-js";

const [n] = createSignal(1);
const over: { sync?: boolean } = { sync: false };
const opts = { sync: true };
const ce = createEffect;

// ------------------------------------------------------------------------------------------------------------------------------ SC7001
// silent, slot-callable: the apply is a function.
export function EffectFunction() {
  createEffect(() => n(), (value) => void value);
  return <div />;
}
// silent, slot-callable: a render effect's apply.
export function RenderEffectFunction() {
  createRenderEffect(() => n(), () => {});
  return <div />;
}
// silent, slot-callable: a bundle whose `effect` is a function.
export function EffectBundle() {
  createEffect(() => n(), { effect: () => {}, error: () => {} });
  return <div />;
}
// silent, slot-callable: the namespace member and a const alias are the primitive.
export function EffectNamespace() {
  S.createEffect(() => n(), () => {});
  ce(() => n(), () => {});
  return <div />;
}
// no site: a local function named like the primitive is not the primitive.
export function EffectShadowed() {
  const createEffect = (compute: () => number, apply: number) => void [compute, apply];
  createEffect(() => n(), 1);
  return <div />;
}
// V: an apply argument no type vouches for (`undefined as any`), the runtime throws MISSING_EFFECT_FN.
export function EffectMissing() {
  createEffect(() => n(), undefined as any);
  return <div />;
}

// ------------------------------------------------------------------------------------------------------------------------------ SC7002
// silent, not-asserted: no options, `sync: false`, an options literal with no `sync`, a literal whose last `sync` is false.
export function MemoNoOptions() {
  const m = createMemo(async () => 1);
  return <div>{String(m())}</div>;
}
export function MemoSyncFalse() {
  const m = createMemo(async () => 1, { sync: false });
  return <div>{String(m())}</div>;
}
export function MemoOtherOption() {
  const m = createMemo(async () => 1, { loadingValue: 0 });
  return <div>{String(m())}</div>;
}
export function MemoDuplicateFalseDirective() {
  // @ts-ignore a directive silences TS1117 for both compilers: the last `sync` decides, as at runtime
  const m = createMemo(async () => 1, { sync: true, sync: false });
  return <div>{String(m())}</div>;
}
// silent, tsc-reports: a duplicate `sync` key no directive silences is TS1117, TypeScript's.
export function MemoDuplicateKey() {
  const m = createMemo(async () => 1, { sync: false, sync: true });
  return <div>{String(m())}</div>;
}
// silent, computation-not-async: a computation proven synchronous, or a value that is no function.
export function MemoSyncComputation() {
  const m = createMemo(() => 1, { sync: true });
  return <div>{String(m())}</div>;
}
export function SignalValue() {
  const [v] = createSignal(0, { sync: true });
  return <div>{String(v())}</div>;
}
// no site: `createTrackedEffect` reads no thenable (its node's compute returns `undefined`), so its `sync` is inert (dist/dev.js:2878-2892).
export function TrackedEffectSync() {
  createTrackedEffect(() => void n(), { sync: true });
  return <div />;
}
// V: `sync: true` over an async computation, in every primitive that takes it.
export function MemoSyncAsync() {
  const m = createMemo(async () => 1, { sync: true });
  return <div>{String(m())}</div>;
}
export function SignalSyncAsync() {
  const [v] = createSignal(async () => 1, { sync: true });
  return <div>{String(v())}</div>;
}
export function OptimisticSyncAsync() {
  const [v] = createOptimistic(async () => 1, { sync: true });
  return <div>{String(v())}</div>;
}
export function EffectSyncAsync() {
  createEffect(async () => 1, () => {}, { sync: true });
  return <div />;
}
export function RenderEffectSyncAsync() {
  createRenderEffect(async () => 1, () => {}, { sync: true });
  return <div />;
}
// U (compute-return-unproven): options the syntax cannot read, or a spread after a literal `true` (the spread may set `sync: false`, which the runtime reads last).
export function MemoOptionsVariable() {
  const m = createMemo(async () => 1, opts);
  return <div>{String(m())}</div>;
}
export function MemoSpreadAfterTrue() {
  const m = createMemo(async () => 1, { sync: true, ...over });
  return <div>{String(m())}</div>;
}
export function SignalSpreadAfterTrue() {
  const [v] = createSignal(async () => 1, { sync: true, ...over });
  return <div>{String(v())}</div>;
}
export function OptimisticSpreadAfterTrue() {
  const [v] = createOptimistic(async () => 1, { sync: true, ...over });
  return <div>{String(v())}</div>;
}
export function EffectSpreadAfterTrue() {
  createEffect(async () => 1, () => {}, { sync: true, ...over });
  return <div />;
}
export function RenderEffectSpreadAfterTrue() {
  createRenderEffect(async () => 1, () => {}, { sync: true, ...over });
  return <div />;
}
