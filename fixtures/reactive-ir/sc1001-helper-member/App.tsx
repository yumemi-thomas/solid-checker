import { createMemo, createProjection, createStore, untrack } from "solid-js";

type S = { v: number };
function useProj() {
  return createProjection<S>(() => {}, { v: 0 });
}

export function ProjHelper() {
  const store = createProjection<S>(() => {}, { v: 0 });
  const r = () => { void store.v; };
  r();
  return <p />;
}

export function ProjTwoLevel() {
  const store = createProjection<S>(() => {}, { v: 0 });
  const r1 = () => { void store.v; };
  const r2 = () => { r1(); };
  r2();
  return <p />;
}

export function ProjDraftHelper() {
  const store = createProjection((d: S) => { d.v = 1; }, { v: 0 });
  const r = () => { void store.v; };
  r();
  return <p />;
}

export function HookProjHelper() {
  const store = useProj();
  const r = () => { void store.v; };
  r();
  return <p />;
}

export function AsHelper() {
  const [store] = createStore<S>({ v: 0 });
  const r = () => { void (store as S).v; };
  r();
  return <p />;
}

export function NonNullHelper() {
  const [store] = createStore<S>({ v: 0 });
  const r = () => { void store!.v; };
  r();
  return <p />;
}

export function SatisfiesHelper() {
  const [store] = createStore<S>({ v: 0 });
  const r = () => { void (store satisfies S).v; };
  r();
  return <p />;
}

// Negatives: the helper runs where the read is tracked or untracked on purpose.
export function ProjHelperInMemo() {
  const store = createProjection<S>(() => {}, { v: 0 });
  const r = () => store.v;
  const m = createMemo(() => r());
  return <p>{m()}</p>;
}

export function ProjHelperUntracked() {
  const store = createProjection<S>(() => {}, { v: 0 });
  const r = () => store.v;
  untrack(r);
  return <p />;
}
