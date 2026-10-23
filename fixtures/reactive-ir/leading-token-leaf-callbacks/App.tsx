import { createMemo, createSignal, onSettled } from "solid-js";

function outer() {
  createMemo(() => 1);
}
outer.cb = () => {};
const cb = outer.cb;

function factory() {
  return () => {
    createMemo(() => 1);
  };
}

function identity(callback: () => void) {
  return callback;
}

// UNCERTIFIABLE: the callback is `outer.cb`, not `outer`; its leading token
// named `outer`, whose body creates a memo.
export function ExpandoMember() {
  onSettled(outer.cb);
  return <div />;
}
// UNCERTIFIABLE: the same member through a closed identity adapter.
export function ExpandoThroughIdentity() {
  onSettled(identity(outer.cb));
  return <div />;
}
// UNCERTIFIABLE: the callback is the bound `factory`, not the arrow it returns.
export function BoundFactory() {
  onSettled(factory.bind(null));
  return <div />;
}
// VIOLATION: the factory call's returned arrow is the callback.
export function CalledFactory() {
  onSettled(factory());
  return <div />;
}
// VIOLATION: the identifier names `outer` itself.
export function IdentifierCallback() {
  onSettled(outer);
  return <div />;
}
// UNCERTIFIABLE: `cb`'s value is no exact function here.
export function BoundMember() {
  onSettled(cb);
  return <div />;
}

// UNCERTIFIABLE: `getFn()()` runs the memo's value, not `getFn`; before, the
// leading token proved the call inert (clean).
export function CalledResult() {
  const getFn = createMemo(() => () => {
    createMemo(() => 1);
  });
  onSettled(() => {
    getFn()();
  });
  return <div />;
}

function makeUpdater() {
  return (prev: number) => {
    createMemo(() => 1);
    return prev;
  };
}
function updater(prev: number) {
  createMemo(() => 1);
  return prev;
}
// UNCERTIFIABLE: the setter runs `makeUpdater()`'s result, not `makeUpdater`
// (whose own body is clean); before, the leaf was certified.
export function UpdaterResult() {
  const [, setN] = createSignal(0);
  onSettled(() => {
    setN(makeUpdater());
  });
  return <div />;
}
// VIOLATION: the setter runs `updater` before it returns.
export function UpdaterIdentifier() {
  const [, setN] = createSignal(0);
  onSettled(() => {
    setN(updater);
  });
  return <div />;
}
