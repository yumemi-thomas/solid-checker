import { createEffect, createSignal } from "solid-js";

const [c] = createSignal(1);
const [w] = createSignal(0);
const chosen = () => ({ read: (): number => 1 });
const pick = Math.random() >= 0 ? chosen : chosen;

function useChain(n: () => number) {
  return { read: (): number => n() };
}
function useCurried(n: () => number) {
  return () => () => n();
}
function useList(n: () => number) {
  return [(): number => n()];
}
function useMembers(n: () => number) {
  return [{ m: (): number => n() }];
}
function usePlain(n: number) {
  return { read: (): number => n };
}

export function OnTheSpot() {
  useChain(c).read();
  useCurried(c)()();
  useList(c).forEach((f) => f());
  const held = useMembers(c);
  held.forEach((e) => e.m());
  pick().read();
  return <p />;
}

export function Clean() {
  usePlain(1).read();
  return <p />;
}

export function Child(props: { f: () => number }) {
  const unknownValue = JSON.parse("null") as (() => number) | null;
  const either = unknownValue ?? props.f;
  let rebound!: () => number;
  rebound = props.f;
  const map = new Map([["f", props.f]]);
  const filled = new Map<string, () => number>();
  filled.set("f", props.f);
  class Holder {
    f = props.f;
  }
  const held = new Holder();
  const iife = (() => props.f)();
  createEffect(
    () => w(),
    () => {
      either();
      rebound();
      map.get("f")!();
      filled.get("f")!();
      held.f();
      iife();
    },
  );
  return <p />;
}

export function App() {
  return <Child f={c} />;
}
