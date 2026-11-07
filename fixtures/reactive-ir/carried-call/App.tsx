import { createEffect, createMemo, createSignal, untrack } from "solid-js";

// ADR 0894: a setter or accessor handed whole to a carrier that provably runs
// it is judged as a call of it at the carrier.
export function ForEachSetter() {
  const [c, setC] = createSignal(0);
  [1].forEach(setC);
  return <div>{c()}</div>;
}

export function ForEachAccessor() {
  const [c] = createSignal(1);
  [1].forEach(c);
  return <div />;
}

export function UntrackLabel() {
  const [c] = createSignal(1);
  void untrack(c, "x");
  return <div />;
}

// The carrier's role is the call's: a handler writes freely, a memo's compute
// does not.
export function HandlerRole() {
  const [c, setC] = createSignal(0);
  return <button onClick={() => [1].forEach(setC)}>{c()}</button>;
}

export function MemoRole() {
  const [c, setC] = createSignal(0);
  const m = createMemo(() => {
    [1].forEach(setC);
    return c();
  });
  return <div>{m()}</div>;
}

// Review D23, negatives: a position no carrier runs, and no run at all.
export function SecondArgument() {
  const [c, setC] = createSignal(0);
  [1].forEach(() => {}, setC);
  return <div>{c()}</div>;
}

export function ReduceInitial() {
  const [c, setC] = createSignal(0);
  [1].reduce((a) => a, setC as unknown as number);
  return <div>{c()}</div>;
}

export function ArrayFromSource() {
  const [c, setC] = createSignal(0);
  Array.from(setC as unknown as number[]);
  return <div>{c()}</div>;
}

export function UntrackSecond(props: { x: () => number }) {
  const [c, setC] = createSignal(0);
  void untrack(props.x, setC as unknown as string);
  return <div>{c()}</div>;
}

export function EmptyReceiver() {
  const [c, setC] = createSignal(0);
  ([] as number[]).forEach(setC);
  return <div>{c()}</div>;
}

// `untrack` with no label sets no strict window; a variable label is unknown.
export function UntrackPlain() {
  const [c] = createSignal(1);
  void untrack(c);
  return <div />;
}

export function UntrackVariableLabel(props: { label: string }) {
  const [c] = createSignal(1);
  void untrack(c, props.label);
  return <div />;
}

// An effect apply: a labelled `untrack` warns there too (Chrome rc.21).
export function UntrackInApply() {
  const [c] = createSignal(1);
  createEffect(
    () => 1,
    () => {
      void untrack(c, "x");
    }
  );
  return <div />;
}
