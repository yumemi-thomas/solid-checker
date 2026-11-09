import { createEffect, createSignal, For } from "solid-js";

// ADR 0920: a read in a callback handed parameter to parameter (`hc1(g)`
// passes `g` to `hc0`, which calls it) inside a window callback. No layer
// places it, so nothing proves it deferred: uncertifiable at the read.

// Uncertifiable: a `For` row (rc.21 warns STRICT_READ_UNTRACKED in Chrome).
export function ForwardedInRow() {
  const [count] = createSignal(3);
  return (
    <For each={[1]}>
      {() => {
        const hc0 = (g: () => void) => { g(); };
        const hc1 = (g: () => void) => { hc0(g); };
        hc1(() => {
          count();
        });
        return <li>x</li>;
      }}
    </For>
  );
}

// Uncertifiable: a ref callback named by a binding.
export function ForwardedInNamedRef() {
  const [count] = createSignal(3);
  const ref = () => {
    const hc0 = (g: () => void) => { g(); };
    const hc1 = (g: () => void) => { hc0(g); };
    hc1(() => {
      count();
    });
  };
  return <p ref={ref}>x</p>;
}

// Uncertifiable: the cleanup a named apply returns, run on the rerun.
export function ForwardedInNamedCleanup() {
  const [count] = createSignal(3);
  const [tick, setTick] = createSignal(1);
  const apply = () => () => {
    const hc0 = (g: () => void) => { g(); };
    const hc1 = (g: () => void) => { hc0(g); };
    hc1(() => {
      count();
    });
  };
  createEffect(() => tick(), apply);
  return <button onClick={() => { setTick(2); }}>go</button>;
}

// Silent: a handler in the row runs when the event fires.
export function HandlerInRow() {
  const [count] = createSignal(3);
  return <For each={[1]}>{() => <button onClick={() => { count(); }}>x</button>}</For>;
}

// Silent: a timer's callback runs on a fresh stack.
export function TimerInRow() {
  const [count] = createSignal(3);
  return (
    <For each={[1]}>
      {() => {
        setTimeout(() => {
          count();
        });
        return <li>x</li>;
      }}
    </For>
  );
}

// Unchanged: the same forwarding in the component body was uncertifiable.
export function ForwardedInBody() {
  const [count] = createSignal(3);
  const hc0 = (g: () => void) => { g(); };
  const hc1 = (g: () => void) => { hc0(g); };
  hc1(() => {
    count();
  });
  return <div>x</div>;
}
