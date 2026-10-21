// Negative: a `var` is initialized only if its initializer executed. A loop
// that skips it, a branch that does not take it, and a throw caught before it
// all leave `late` undefined when `go()` calls it (TypeError, body never runs).
// Only a `const` or `let` is credited from source position.
import { createSignal } from "solid-js";

const [, setCount] = createSignal(0);
export function InLoop() {
  for (let i = 0; i < 1; i++) {
    if (i === 0) continue;
    var late = () => {
      setCount(1);
    };
  }
  function go() {
    late();
  }
  go();
  return <button />;
}
export function InBranch() {
  if (false) {
    var late = () => {
      setCount(2);
    };
  }
  function go() {
    late();
  }
  go();
  return <button />;
}
export function Caught() {
  try {
    throw 0;
    var late = () => {
      setCount(3);
    };
  } catch {}
  function go() {
    late();
  }
  go();
  return <button />;
}
// Positive: the same helper bound by a `const` is proven.
export function Control() {
  const late = () => {
    setCount(4);
  };
  function go() {
    late();
  }
  go();
  return <button />;
}
