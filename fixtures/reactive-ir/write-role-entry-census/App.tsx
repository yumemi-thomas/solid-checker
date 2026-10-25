import { createEffect, createMemo, createSignal } from "solid-js";

// Calls its `run` prop in its own body, under the component's owner.
function RunsInBody(props: { run: () => void }) {
  props.run();
  return <p />;
}

// Keeps a callback it is handed; nothing here calls it.
let kept: (() => void) | undefined;
function keep(callback: () => void) {
  kept = callback;
}

// Violation: `reset` is also a prop `RunsInBody` calls in its body, where the
// rc.17 runtime throws REACTIVE_WRITE_IN_OWNED_SCOPE (ADR 0400 follows the prop
// into the child). The handler call is one of its entries, not all of them.
export function EscapedToChild() {
  const [n, setN] = createSignal(0);
  const reset = () => setN(0);
  return (
    <>
      <button onClick={() => reset()}>{n()}</button>
      <RunsInBody run={reset} />
    </>
  );
}

// Violation: the same prop call, beside a handler named by the attribute.
export function NamedHandlerEscapedToChild() {
  const [n, setN] = createSignal(0);
  const reset = () => setN(0);
  return (
    <>
      <button onClick={reset}>{n()}</button>
      <RunsInBody run={reset} />
    </>
  );
}

// Not clean: `keep` stores `reset`; whoever calls it later is not followed.
export function StoredByHelper() {
  const [n, setN] = createSignal(0);
  const reset = () => setN(0);
  keep(reset);
  return <button onClick={() => reset()}>{n()}</button>;
}

// Violation: the alias is followed to the prop `RunsInBody` calls in its body.
export function Aliased() {
  const [n, setN] = createSignal(0);
  const reset = () => setN(0);
  const again = reset;
  return (
    <>
      <button onClick={() => reset()}>{n()}</button>
      <RunsInBody run={again} />
    </>
  );
}

// Violation: a named handler that the body also calls.
export function NamedHandlerCalledInBody() {
  const [n, setN] = createSignal(0);
  const reset = () => setN(0);
  reset();
  return <button onClick={reset}>{n()}</button>;
}

// Violation: `createMemo` runs `reset` as its compute, in an owned
// computation, though the effect also names it as its apply.
export function MemoAndEffectApply() {
  const [n, setN] = createSignal(0);
  const reset = () => {
    setN(0);
  };
  createEffect(() => n(), reset);
  const memo = createMemo(reset);
  return <button onClick={() => memo()}>{n()}</button>;
}

// Clean: the effect's apply is the only entry of `reset`.
export function EffectApplyOnly() {
  const [n, setN] = createSignal(0);
  const reset = () => {
    setN(0);
  };
  createEffect(() => n(), reset);
  return <p>{n()}</p>;
}

// Clean: every entry of `reset` is a call in a click handler.
export function HandlerCallsOnly() {
  const [n, setN] = createSignal(0);
  const reset = () => setN(0);
  return (
    <>
      <button onClick={() => reset()}>{n()}</button>
      <button onDblClick={() => reset()}>{n()}</button>
    </>
  );
}

// Clean: `reset` is only ever an event handler, named or called.
export function NamedHandlerOnly() {
  const [n, setN] = createSignal(0);
  const reset = () => setN(0);
  return (
    <button onClick={reset} onDblClick={() => reset()}>
      {n()}
    </button>
  );
}

// Clean: the `reset` handed to `RunsInBody` is the parameter that shadows the
// writing `reset`, which only the click handler calls.
export function Shadowed() {
  const [n, setN] = createSignal(0);
  const reset = () => setN(0);
  const child = (reset: () => void) => <RunsInBody run={reset} />;
  return (
    <>
      <button onClick={() => reset()}>{n()}</button>
      {child(() => {})}
    </>
  );
}

// Clean: a type query names `reset` without entering it.
export function TypeQuery() {
  const [n, setN] = createSignal(0);
  const reset = () => setN(0);
  const handlers: { reset: typeof reset }[] = [];
  return <button onClick={() => reset()}>{n()}{handlers.length}</button>;
}

const [total, setTotal] = createSignal(0);

// Not clean in this open program: modules this build does not see may call the
// export anywhere, an owned scope included.
export const resetTotal = () => setTotal(0);

export function ExportedHelper() {
  return <button onClick={() => resetTotal()}>{total()}</button>;
}
