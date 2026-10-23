import { createEffect, createMemo, createSignal } from "solid-js";

// Calls its `run` prop in its own body, under the component's owner.
function Child(props: { run: () => void }) {
  props.run();
  return <p />;
}

// No violation: `reset` is reassigned. The effect keeps the original writer as
// its apply, which is legal; the memo gets the no-op. The binding names two
// functions, so neither position is proof about the writer.
export function ReassignedCallback() {
  const [n, setN] = createSignal(0);
  let reset = () => {
    setN(1);
  };
  createEffect(() => n(), reset);
  reset = () => {};
  const memo = createMemo(reset);
  return <button onClick={() => memo()}>{n()}</button>;
}

// Not clean: the writer escapes through its outer binding `reset`, though its
// own name `inner` is only its declaration. `Child` runs it in its body.
export function NamedFunctionExpression() {
  const [n, setN] = createSignal(0);
  const reset = function inner() {
    setN(1);
  };
  return (
    <>
      <button onClick={() => reset()}>{n()}</button>
      <Child run={reset} />
    </>
  );
}

// Not clean: `<Reset />` renders the handler under a fresh component owner,
// where the write throws. A recursive render is no call in the caller's role.
let again = false;
export function RecursiveRender() {
  const [n, setN] = createSignal(0);
  const Reset: () => any = () => {
    setN(1);
    if (again) return <p />;
    again = true;
    return <Reset />;
  };
  return <button onClick={Reset}>{n()}</button>;
}

// Not clean: the click handler's entries say nothing of the closure it
// creates and hands to `Child`, which calls it in its body.
export function NestedEscape() {
  const [n, setN] = createSignal(0);
  const click = () => {
    const reset = () => {
      setN(1);
    };
    return <Child run={reset} />;
  };
  return <button onClick={click}>{n()}</button>;
}

// Clean: the closure runs inline in the click handler (`forEach` calls it
// during the call), so the handler's entries are its entries.
export function NestedInline() {
  const [n, setN] = createSignal(0);
  const click = () => {
    [1].forEach((value) => setN(value));
  };
  return <button onClick={click}>{n()}</button>;
}

// Clean: `setTimeout` runs the closure on a fresh stack, which no owner
// reaches, wherever the handler scheduled it from.
export function NestedTimer() {
  const [n, setN] = createSignal(0);
  const click = () => {
    setTimeout(() => setN(0), 10);
  };
  return <button onClick={click}>{n()}</button>;
}
