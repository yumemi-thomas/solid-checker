import { createSignal, onSettled } from "solid-js";

// ADR 1204: an earlier evaluation in the body throws (a getter), so the read after it never runs: Chrome reports no
// STRICT_READ_UNTRACKED in HandedThrows, DirectThrows and VoidThrows. The controls' getters return, and the read warns.
export function HandedThrows() {
  const [s] = createSignal(1);
  const f = () => {
    s();
  };
  const box = {
    get cb(): () => void {
      throw new Error("no callback");
    },
  };
  onSettled(box.cb);
  f();
  return <div />;
}

export function DirectThrows() {
  const [s] = createSignal(1);
  const box = {
    get cb(): () => void {
      throw new Error("no callback");
    },
  };
  onSettled(box.cb);
  s();
  return <div />;
}

export function VoidThrows() {
  const [s] = createSignal(1);
  const f = () => {
    s();
  };
  const box = {
    get cb(): () => void {
      throw new Error("no callback");
    },
  };
  void box.cb;
  f();
  return <div />;
}

export function HandedReturns() {
  const [s] = createSignal(1);
  const f = () => {
    s();
  };
  const box = {
    get cb(): () => void {
      return () => {};
    },
  };
  onSettled(box.cb);
  f();
  return <div />;
}
