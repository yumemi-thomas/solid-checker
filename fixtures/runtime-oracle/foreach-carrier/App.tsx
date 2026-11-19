import { createEffect, createMemo, createSignal, onSettled } from "solid-js";

// ADR 1124: a whole element of a spread-free array literal the standard `Array.prototype.forEach` runs, through an inline arrow
// whose one parameter is only called, is a call of the callback at that `forEach` call, classified as a read written there.

// Clean: the carried run is in a click handler or a timer, where no read rule reports.
export function MemoCarriedInHandler() {
  const [s] = createSignal(1);
  const cb = () => s();
  const m = createMemo(cb);
  return <button onClick={() => [cb].forEach((g) => g())}>{m()}</button>;
}

export function SettledCarriedInTimer() {
  const [s] = createSignal(1);
  const cb = () => {
    s();
  };
  onSettled(cb);
  setTimeout(() => [cb].forEach((g) => g()), 0);
  return <div />;
}

export function EffectComputeCarriedInHandler() {
  const [s] = createSignal(1);
  const cb = () => s();
  createEffect(cb, () => {});
  return <button onClick={() => [cb].forEach((g) => g())}>x</button>;
}

// Uncertifiable: the carried run is in the body or an effect's apply, where the read warns.
export function MemoCarriedInBody() {
  const [s] = createSignal(1);
  const cb = () => s();
  const m = createMemo(cb);
  [cb].forEach((g) => g());
  return <div>{m()}</div>;
}

export function MemoCarriedInApply() {
  const [s] = createSignal(1);
  const cb = () => s();
  const m = createMemo(cb);
  createEffect(() => 1, () => {
    [cb].forEach((g) => g());
  });
  return <div>{m()}</div>;
}

// Uncertifiable twins: the reference is no carried call.
export function TwinMap() {
  const [s] = createSignal(1);
  const cb = () => s();
  const m = createMemo(cb);
  return <button onClick={() => [cb].map((g) => g())}>{m()}</button>;
}

export function TwinNonLiteral() {
  const [s] = createSignal(1);
  const cb = () => s();
  const m = createMemo(cb);
  const list = [cb];
  return <button onClick={() => list.forEach((g) => g())}>{m()}</button>;
}

export function TwinReassigned() {
  const [s] = createSignal(1);
  const cb = () => s();
  const m = createMemo(cb);
  return <button onClick={() => [cb].forEach((g) => { g = () => 0; g(); })}>{m()}</button>;
}

export function TwinPassedOn() {
  const [s] = createSignal(1);
  const cb = () => s();
  const m = createMemo(cb);
  return <button onClick={() => [cb].forEach((g) => { setTimeout(g, 0); })}>{m()}</button>;
}

export function TwinNested() {
  const [s] = createSignal(1);
  const cb = () => s();
  const m = createMemo(cb);
  return <button onClick={() => [cb].forEach((g) => { queueMicrotask(() => g()); })}>{m()}</button>;
}

export function TwinSpread() {
  const [s] = createSignal(1);
  const cb = () => s();
  const m = createMemo(cb);
  return <button onClick={() => [...[cb]].forEach((g) => g())}>{m()}</button>;
}

export function TwinSecondParameter() {
  const [s] = createSignal(1);
  const cb = () => s();
  const m = createMemo(cb);
  return <button onClick={() => [cb].forEach((g, i, a) => { void i; a[0](); })}>{m()}</button>;
}

export function TwinAlias() {
  const [s] = createSignal(1);
  const cb = () => s();
  const m = createMemo(cb);
  const c1 = cb;
  return <button onClick={() => [c1].forEach((g) => g())}>{m()}</button>;
}
