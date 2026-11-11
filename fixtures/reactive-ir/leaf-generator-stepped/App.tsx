import { createMemo, createTrackedEffect, onSettled } from "solid-js";

// ADR 0961 (batch-10 addendum 12, group B): a generator's body runs where its iterator is stepped. A leaf callback that keeps the
// iterator a call of a generator returns (`.next()`, a `for…of`, a spread, `Array.from`, a stored loop, `yield*` through another
// generator, an async generator's `.next()`) may run the body in the leaf scope, where Chrome (rc.21) throws
// PRIMITIVE_IN_FORBIDDEN_SCOPE at the creation: no walk follows the stepping, so the call is an uncertifiable row of SC3001
// (`generator-stepped`), never silence. A generator whose body holds a call is a row; one with none has nothing to run.
function* makes() {
  createMemo(() => 1);
  yield 1;
}
function* delegates() {
  yield* makes();
}
async function* makesAsync() {
  createMemo(() => 1);
  yield 1;
}

export function Next() {
  onSettled(() => {
    void makes().next();
  });
  return <p>x</p>;
}
export function ForOf() {
  createTrackedEffect(() => {
    for (const _ of makes()) void _;
  });
  return <p>x</p>;
}
export function Spread() {
  onSettled(() => {
    void [...makes()];
  });
  return <p>x</p>;
}
export function FromArray() {
  onSettled(() => {
    void Array.from(makes());
  });
  return <p>x</p>;
}
export function StoredLoop() {
  onSettled(() => {
    const steps = makes();
    for (const _ of steps) void _;
  });
  return <p>x</p>;
}
export function Delegated() {
  onSettled(() => {
    for (const _ of delegates()) void _;
  });
  return <p>x</p>;
}
export function AsyncNext() {
  onSettled(() => {
    void makesAsync().next();
  });
  return <p>x</p>;
}

// The plain function: the walk's own proof (a violation).
function plainMakes() {
  createMemo(() => 1);
}
export function Plain() {
  onSettled(() => {
    plainMakes();
  });
  return <p>x</p>;
}

// Silent: a call a statement discards runs none of the body.
export function Discarded() {
  onSettled(() => {
    makes();
  });
  return <p>x</p>;
}
// Silent: a generator with no call in its body has nothing to run.
function* quiet() {
  yield 1;
}
export function Quiet() {
  onSettled(() => {
    for (const _ of quiet()) void _;
  });
  return <p>x</p>;
}
// Silent: stepped by a handler, outside any leaf scope.
export function InHandler() {
  return (
    <button
      onClick={() => {
        for (const _ of makes()) void _;
      }}
    >
      x
    </button>
  );
}
