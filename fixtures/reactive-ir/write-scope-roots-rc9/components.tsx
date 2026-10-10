import {
  createMemo,
  createOptimistic,
  createOptimisticStore,
  createSignal,
  createStore,
  flush,
  untrack,
} from "solid-js";

const [count, setCount] = createSignal(0);
const [state, setState] = createStore({ value: 0 });
const [pending, setPending] = createOptimistic(0);
const [draft, setDraft] = createOptimisticStore({ value: 0 });

// In dev `createComponent` runs every component under
// `createRoot(() => untrack(() => Comp(props)), { transparent: true })`, so a
// write directly in a component body meets its guard exactly as one directly
// in a `createRoot` body does.

function writeStoreFromCounter() {
  // Called only from the component body: the same answer as the body.
  setState((s) => {
    s.value = 1;
  });
}

function Counter() {
  // SC2001 on every release: signal setters never exempted roots.
  setCount(1);
  setPending(1);
  // SC2001 only where the store setter's guard rejects roots (rc.9).
  setState((s) => {
    s.value = 2;
  });
  setDraft((s) => {
    s.value = 2;
  });
  // `untrack` and `flush(fn)` keep the component's owner: as the body.
  untrack(() =>
    setState((s) => {
      s.value = 3;
    }),
  );
  flush(() =>
    setDraft((s) => {
      s.value = 3;
    }),
  );
  writeStoreFromCounter();
  // A memo compute in the body is the memo's scope, not the component's root:
  // SC2001 on every release.
  createMemo(() => {
    setState((s) => {
      s.value = 4;
    });
    return count();
  });
  return null;
}

// A nested component's body is its own root: the same answers.
function Row() {
  // SC2001 on every release.
  setCount(2);
  // SC2001 only where the store setter's guard rejects roots (rc.9).
  setState((s) => {
    s.value = 5;
  });
  return null;
}

function List() {
  return <Row />;
}

export function App() {
  return (
    <>
      <Counter />
      <List />
    </>
  );
}

export { draft, pending, state };
