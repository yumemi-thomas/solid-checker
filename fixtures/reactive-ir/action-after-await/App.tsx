import { action, createSignal } from "solid-js";

// Proved: after `.then`, the action's body only calls a setter with a literal.
export function Plain() {
  const [w1, setW1] = createSignal(0);
  const a1 = action(function* () { setW1(7); });
  Promise.resolve().then(() => {
    a1();
  });
  return <div>{w1()}</div>;
}

// Open: the body reads a signal.
export function ReadsSignal() {
  const [count] = createSignal(1);
  const [w2, setW2] = createSignal(0);
  const a2 = action(function* () { setW2(count()); });
  Promise.resolve().then(() => {
    a2();
  });
  return <div>{w2()}</div>;
}

// Open: the body yields, and its next step is the action's own continuation.
export function Yields() {
  const [w3, setW3] = createSignal(0);
  const a3 = action(function* () { setW3(1); yield Promise.resolve(); setW3(2); });
  Promise.resolve().then(() => {
    a3();
  });
  return <div>{w3()}</div>;
}

// Open: the action is held by a `let` the program writes.
export function Rebound() {
  const [w4, setW4] = createSignal(0);
  let a4 = action(function* () { setW4(7); });
  a4 = action(function* () { setW4(8); });
  Promise.resolve().then(() => {
    a4();
  });
  return <div>{w4()}</div>;
}
