import { createSignal } from "solid-js";

// Stepped in a handler: the read runs on `it1.next()`, never on `g1()`, so its row is at the read.
export function Stepped() {
  const [r1] = createSignal(3);
  function* g1() {
    yield 1;
    r1();
  }
  const it1 = g1();
  it1.next();
  return <button onClick={() => { it1.next(); }}>go</button>;
}

// Stepped by a destructuring: the same, at the read.
export function Destructured() {
  const [r2] = createSignal(3);
  function* g2() {
    yield 1;
    r2();
  }
  const [, d] = g2();
  void d;
  return <p />;
}

// Not a generator: the call runs the helper's body, and the row stays at the call.
export function Helper() {
  const [r3] = createSignal(3);
  const h = () => {
    r3();
  };
  h();
  return <p />;
}
