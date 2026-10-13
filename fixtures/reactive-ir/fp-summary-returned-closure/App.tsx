import { createSignal } from "solid-js";

// A hook whose result carries closures over its own signal. Calling the hook
// reads nothing of `state`: each closure reads it when the closure runs.
export function createMutation() {
  const [state, setState] = createSignal<{ kind: string }>({ kind: "idle" });
  const run = () => {
    setState({ kind: "running" });
  };
  return {
    state,
    run,
    busy: () => state().kind === "running",
  };
}

// ---- Negatives: the hook is called in the body, its closures are not.

// The closure is read in a JSX attribute, which tracks.
export function Screen() {
  const m = createMutation();
  return (
    <button disabled={m.busy()} onClick={() => m.run()}>
      go
    </button>
  );
}

// A property closure with a block body, only invoked from a handler.
function createResetter() {
  const [s, set] = createSignal(0);
  return {
    reset: () => {
      if (s() > 0) set(0);
    },
  };
}
export function Resetter() {
  const r = createResetter();
  return <button onClick={() => r.reset()}>reset</button>;
}

// A tuple element closure.
function createFlag() {
  const [s] = createSignal(0);
  return [s, () => s() > 0] as const;
}
export function Flag() {
  const [value, positive] = createFlag();
  return <span>{positive() ? value() : 0}</span>;
}

// A closure that calls a project helper.
function createGuarded() {
  const [s] = createSignal(0);
  const isOn = () => s() > 0;
  return { on: () => isOn() };
}
export function Guarded() {
  const g = createGuarded();
  return <span>{g.on()}</span>;
}

// A getter is a function of its own, run when the property is read.
function createGetter() {
  const [s] = createSignal(0);
  return {
    get big() {
      return s() > 10;
    },
  };
}
export function Getter() {
  const g = createGetter();
  return <span>{g.big}</span>;
}

// A method.
function createMethod() {
  const [s] = createSignal(0);
  return {
    small() {
      return s() < 3;
    },
  };
}
export function Method() {
  const m = createMethod();
  return <span>{m.small()}</span>;
}

// ---- Positive controls: the hook itself reads while the body runs.

// The hook reads its signal directly while it is called.
function createSnapshot() {
  const [s] = createSignal(0);
  const first = s();
  return { first, later: () => s() + 1 };
}
export function Snapshot() {
  const snap = createSnapshot();
  return <span>{snap.first}</span>;
}

// A hook returning a closure that is *also* invoked immediately by the hook.
function createEager() {
  const [s] = createSignal(0);
  const read = () => s() + 1;
  const now = read();
  return { read, now };
}
export function Eager() {
  const e = createEager();
  return <span>{e.now}</span>;
}
