import { createSignal } from "solid-js";
import {
  createBoundMutation,
  createHeldMutation,
  createLateMutation,
  createMemoMutation,
  createMutation,
  createOpaqueMutation,
  createShadowedMutation,
  createStoreMutation,
  createThisMutation,
  createWrappedMutation,
} from "./mutation";

declare function send(value: number): Promise<number>;
declare const pending: Promise<void>;

// Positive: the callback runs where `run` calls it, before `run`'s first
// await, in the click handler; `save.state()` is the signal's accessor and
// cannot use `save` as `this`. A read in a handler is legal: quiet.
export function Save() {
  const [count] = createSignal(0);
  const save = createMutation(() => send(count()));
  return (
    <button onClick={() => save.run()} disabled={save.busy()}>
      {save.state().kind}
    </button>
  );
}

// Control: the same helper with `run` called in the component body, so the
// callback's read runs in the body: the strict-read violation, placed exactly.
export function SaveInBody() {
  const [count] = createSignal(0);
  const save = createMutation(() => send(count()));
  void save.run();
  return <div>{save.state().kind}</div>;
}

// An async callback's read after its own await: no invocation places a
// continuation (ADR 0319), so the read stays an obligation.
export function SaveAfterAwait() {
  const [count] = createSignal(0);
  const save = createMutation(async () => {
    await pending;
    return send(count());
  });
  return <button onClick={() => save.run()}>{save.state().kind}</button>;
}

// `run` calls the callback after its own await: the read stays an obligation.
export function SaveLate() {
  const [count] = createSignal(0);
  const save = createLateMutation(() => send(count()));
  return <button onClick={() => save.run()}>{save.state().kind}</button>;
}

// The setter and a `createMemo` accessor as members: quiet.
export function SaveMemo() {
  const [count] = createSignal(0);
  const save = createMemoMutation(() => send(count()));
  return (
    <button onClick={() => save.run()} onDblClick={() => save.set({ kind: "idle" })}>
      {String(save.busy())}
    </button>
  );
}

// A store setter as a member: quiet.
export function SaveStore() {
  const [count] = createSignal(0);
  const save = createStoreMutation(() => send(count()));
  return (
    <button onClick={() => save.run()} onDblClick={() => save.set(() => ({ kind: "idle" }))}>
      {save.store.kind}
    </button>
  );
}

// An arrow wrapping the accessor ignores its receiver by the existing rule.
export function SaveWrapped() {
  const [count] = createSignal(0);
  const save = createWrappedMutation(() => send(count()));
  return <button onClick={() => save.run()}>{save.state().kind}</button>;
}

// Obligations: a method that reads `this`, a `bind`, a held tuple, an
// unresolved factory, a shadowed `createSignal`.
export function SaveThis() {
  const [count] = createSignal(0);
  const save = createThisMutation(() => send(count()));
  return <button onClick={() => save.run()}>{save.state().kind}</button>;
}

export function SaveBound() {
  const [count] = createSignal(0);
  const save = createBoundMutation(() => send(count()));
  return <button onClick={() => save.run()}>{save.state().kind}</button>;
}

export function SaveHeld() {
  const [count] = createSignal(0);
  const save = createHeldMutation(() => send(count()));
  return <button onClick={() => save.run()}>{save.state().kind}</button>;
}

export function SaveOpaque() {
  const [count] = createSignal(0);
  const save = createOpaqueMutation(() => send(count()));
  return <button onClick={() => save.run()}>{save.state().kind}</button>;
}

export function SaveShadowed() {
  const [count] = createSignal(0);
  const save = createShadowedMutation(() => send(count()));
  return <button onClick={() => save.run()}>{save.state().kind}</button>;
}
