import { action, createEffect, createMemo, createRenderEffect, createSignal, onSettled, untrack } from "solid-js";

// Proved: every reference to `cb` is the memo's compute position.
export function MemoOnly() {
  const [s] = createSignal(1);
  const cb = () => s();
  const m = createMemo(cb);
  return <div>{m()}</div>;
}

// Proved: the alias's only reference is the position (ADR 1119 follows a `const` alias).
export function MemoThroughAlias() {
  const [s] = createSignal(1);
  const cb = () => s();
  const cb1 = cb;
  const m = createMemo(cb1);
  return <div>{m()}</div>;
}

// Proved: two positions, both the callback's (`untrack` reports nothing here either).
export function MemoAndUntrack() {
  const [s] = createSignal(1);
  const cb = () => s();
  const m = createMemo(cb);
  untrack(cb);
  return <div>{m()}</div>;
}

// Uncertifiable: the array element is no position; `forEach` runs `cb` in the body (Chrome: STRICT_READ_UNTRACKED).
export function MemoAndCarrier() {
  const [s] = createSignal(1);
  const cb = () => s();
  const m = createMemo(cb);
  [cb].forEach((g) => g());
  return <div>{m()}</div>;
}

// Uncertifiable: a global store is an entry the program does not see.
export function MemoAndGlobal() {
  const [s] = createSignal(1);
  const cb = () => s();
  const m = createMemo(cb);
  (globalThis as { f?: () => number }).f = cb;
  return <div>{m()}</div>;
}

// Uncertifiable: the alias escapes through the array.
export function AliasEscapes() {
  const [s] = createSignal(1);
  const cb = () => s();
  const cb1 = cb;
  const m = createMemo(cb1);
  [cb1].forEach((g) => g());
  return <div>{m()}</div>;
}

// Uncertifiable: a named handler also run by the body (Chrome: STRICT_READ_UNTRACKED).
export function HandlerAndCarrier() {
  const [s] = createSignal(1);
  const cb = () => {
    s();
  };
  [cb].forEach((g) => g());
  return <button onClick={cb}>x</button>;
}

// Uncertifiable: a named settled callback also run by the body (Chrome: STRICT_READ_UNTRACKED).
export function SettledAndCarrier() {
  const [s] = createSignal(1);
  const cb = () => {
    s();
  };
  onSettled(cb);
  [cb].forEach((g) => g());
  return <div />;
}

// Round 2 (H1): an effect's or a render effect's compute named by its binding, also run in the body.
export function EffectComputeAndCarrier() {
  const [s] = createSignal(1);
  const cb = () => s();
  createEffect(cb, () => {});
  [cb].forEach((g) => g());
  return <div />;
}

export function RenderEffectComputeAndCarrier() {
  const [s] = createSignal(1);
  const cb = () => s();
  createRenderEffect(cb, () => {});
  [cb].forEach((g) => g());
  return <div />;
}

export function EffectComputeAliasAndCarrier() {
  const [s] = createSignal(1);
  const cb = () => s();
  const c1 = cb;
  createEffect(c1, () => {});
  [cb].forEach((g) => g());
  return <div />;
}

// Proved: the compute position is the only entry.
export function EffectComputeOnly() {
  const [s] = createSignal(1);
  const cb = () => s();
  createEffect(cb, () => {});
  return <div />;
}

// Round 2 (H2): a settled callback also called by name in an action an effect's apply steps.
export function SettledAndCalledInAction() {
  const [s] = createSignal(1);
  const f = () => {
    void s();
  };
  const a1 = action(function* () {
    onSettled(f);
    f();
  });
  createEffect(
    () => 1,
    () => {
      a1();
    },
  );
  return <div />;
}
