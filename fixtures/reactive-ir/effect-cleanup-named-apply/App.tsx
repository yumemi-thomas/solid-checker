import { createEffect, createSignal } from "solid-js";

// ADR 1200: an apply named by a `const` whose one reference is the effect's apply argument is the inline apply: no other code can call
// it, so its returned cleanup runs only from the effect (ADRs 1037, 1167, 1183).

// Clean: a write in the cleanup is legal on every run of it (ADR 1037).
export function NamedWrite() {
  const [, setW] = createSignal(0);
  const [t, setT] = createSignal(1);
  const apply = () => () => {
    setW(7);
  };
  createEffect(() => t(), apply);
  return <button onClick={() => setT(2)}>go</button>;
}

// Violation: a reached click reruns the effect, which runs the cleanup in the apply's window (ADR 1183).
export function NamedRerunRead() {
  const [r] = createSignal(3);
  const [t, setT] = createSignal(1);
  const apply = () => () => {
    r();
  };
  createEffect(() => t(), apply);
  return <button onClick={() => setT(2)}>go</button>;
}

// Violation: the read forwarded through two in-place helpers in the named apply's cleanup, on the reached rerun (as
// forwarded-callback-nested-window's ForwardedInNamedCleanup).
export function NamedForwardedRead() {
  const [r] = createSignal(3);
  const [t, setT] = createSignal(1);
  const apply = () => () => {
    const hc0 = (g: () => void) => {
      g();
    };
    const hc1 = (g: () => void) => {
      hc0(g);
    };
    hc1(() => {
      r();
    });
  };
  createEffect(() => t(), apply);
  return <button onClick={() => setT(2)}>go</button>;
}

// Silent: the compute tracks nothing, so only the disposal runs the cleanup (ADR 1167).
export function NamedDisposalRead() {
  const [r] = createSignal(3);
  const apply = () => () => {
    r();
  };
  createEffect(() => 1, apply);
  return <p />;
}

// Uncertifiable: the apply is also called elsewhere, which can run its cleanup on that stack.
export function NamedCalledElsewhere() {
  const [, setW] = createSignal(0);
  const [t, setT] = createSignal(1);
  const apply = () => () => {
    setW(7);
  };
  createEffect(() => t(), apply);
  apply()();
  return <button onClick={() => setT(2)}>go</button>;
}

// Uncertifiable: the apply is exported, so unseen code may call it.
export const exportedApply = () => () => {
  void 0;
};
export function NamedExported() {
  const [t, setT] = createSignal(1);
  createEffect(() => t(), exportedApply);
  return <button onClick={() => setT(2)}>go</button>;
}

// Uncertifiable: the apply is handed to two effects.
export function NamedTwice() {
  const [r] = createSignal(3);
  const [t, setT] = createSignal(1);
  const apply = () => () => {
    r();
  };
  createEffect(() => t(), apply);
  createEffect(() => t(), apply);
  return <button onClick={() => setT(2)}>go</button>;
}
