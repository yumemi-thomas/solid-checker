import { createEffect, createSignal, For } from "solid-js";

// ADR 0931: a read in a callback handed parameter to parameter (`hc1(g)`
// passes `g` to `hc0`, which calls it) runs in place: in a window callback
// it is the window's read (ADR 0920 left it uncertifiable).

// Violation: a `For` row (rc.21 warns STRICT_READ_UNTRACKED in Chrome).
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

// Violation: a ref callback named by a binding.
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

// Violation: the same forwarding in the component body.
export function ForwardedInBody() {
  const [count] = createSignal(3);
  const hc0 = (g: () => void) => { g(); };
  const hc1 = (g: () => void) => { hc0(g); };
  hc1(() => {
    count();
  });
  return <div>x</div>;
}
