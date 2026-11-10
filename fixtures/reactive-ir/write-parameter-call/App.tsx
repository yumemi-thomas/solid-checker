import { action, createMemo, createSignal } from "solid-js";

const flag = Math.random() > 0.5;

function run(g: (v: number) => void) {
  g(1);
  return 1;
}

// A setter handed to a function that calls its parameter on every completion
// writes during that call: rc.21 warns in the memo's compute.
export function Declared() {
  const [count, setCount] = createSignal(0);
  const m = createMemo(() => run(setCount) + count());
  return <i>{m()}</i>;
}
// A local arrow, the signal created in the compute.
export function Local() {
  const m = createMemo(() => {
    const call = (g: (v: number) => void) => {
      g(1);
      return 1;
    };
    const [, w] = createSignal(0);
    return call(w);
  });
  return <i>{m()}</i>;
}
// An action handed the same way.
export function Action() {
  const act = action(function* () {
    yield 1;
  });
  const m = createMemo(() => {
    const call = (g: () => unknown) => {
      g();
      return 1;
    };
    return call(act);
  });
  return <i>{m()}</i>;
}
// Called on some paths only: an obligation at the call.
export function SomePaths() {
  const [, setCount] = createSignal(0);
  const m = createMemo(() => {
    const maybe = (g: (v: number) => void) => {
      if (flag) g(1);
      return 1;
    };
    return maybe(setCount);
  });
  return <i>{m()}</i>;
}
// Clean: a closure calls it later, or nothing calls it.
export function Later() {
  const [, setCount] = createSignal(0);
  const m = createMemo(() => {
    const later = (g: (v: number) => void) => () => g(1);
    later(setCount);
    const keep = (_g: (v: number) => void) => 1;
    return keep(setCount);
  });
  return <i>{m()}</i>;
}
// Clean: in an event handler the write is legal.
export function Handler() {
  const [, setCount] = createSignal(0);
  return <i onClick={() => run(setCount)} />;
}
