import { createEffect, createMemo, createSignal } from "solid-js";

// Obligations (SC5008, caught-read-unproven): a memo may throw its stored error into the pass, through a helper or two.
export function ThroughHelper() {
  const m = createMemo(() => 1);
  const read = () => m();
  createEffect(() => {
    try {
      return read();
    } catch {
      return 0;
    }
  }, () => {});
  return <div></div>;
}

export function ThroughTwoHelpers() {
  const m = createMemo(() => 1);
  const read = () => m();
  const again = () => read();
  createEffect(() => {
    try {
      return again();
    } catch {
      return 0;
    }
  }, () => {});
  return <div></div>;
}

// Silent: a plain signal cannot throw into a pass, directly or through a helper.
export function SignalThroughHelper() {
  const [s] = createSignal(1);
  const read = () => s();
  createEffect(() => {
    try {
      return read();
    } catch {
      return 0;
    }
  }, () => {});
  return <div></div>;
}

// Silent: a helper that reads nothing reactive.
export function PlainHelper() {
  const read = () => 1;
  createEffect(() => {
    try {
      return read();
    } catch {
      return 0;
    }
  }, () => {});
  return <div></div>;
}
