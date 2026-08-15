import { createEffect, createMemo, createSignal } from "solid-js";

const [count, setCount] = createSignal(0);

function directive() {
  setCount(1);
  return element => {
    setCount(2);
    // Value-form state: no owner needed, silent (negative).
    createSignal(element);
    // Owner-attaching computation: leaks once per element (positive).
    createMemo(() => count() + 1);
  };
}

function innerDirective() {
  // Value-form state through a forwarded factory: still silent (negative).
  return element => createSignal(element);
}

function forwardedDirective() {
  return innerDirective();
}

function innerComputation() {
  // Owner-attaching effect through a forwarded factory (positive).
  return element => createEffect(() => count(), () => {});
}

function forwardedComputation() {
  return innerComputation();
}

function makeHandler() {
  setCount(4);
  return () => {};
}

export function App() {
  return <button
    ref={[directive(), forwardedDirective(), forwardedComputation(), element => {
      setCount(3);
      // Value-form state (negative) next to a function-form signal, which
      // registers a derived computation (positive).
      createSignal(element);
      createSignal(() => count());
    }]}
    onClick={makeHandler()}
  >{count()}</button>;
}
