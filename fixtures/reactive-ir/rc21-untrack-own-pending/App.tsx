import { createMemo, createProjection, createRoot, createStore, untrack } from "solid-js";

// rc.21: an unlabelled untrack keeps the pending throw for a source the
// strict body itself created (`strictPending`, `owner._parent === context`).
export function Own() {
  const memo = createMemo(async () => 1);
  const [store] = createStore(async () => ({ a: 1 }), { a: 0 });
  const view = createProjection<{ a: number }>(async (draft) => {
    await 0;
    draft.a = 1;
  }, { a: 0 });
  const one = untrack(() => memo());
  const two = untrack(() => store.a);
  const three = untrack(() => view.a);
  const four = untrack(() => untrack(() => memo()));
  return <p>{[one, two, three, four].length}</p>;
}

// A settled source reads silently.
export function Settled() {
  const memo = createMemo(() => 1);
  const value = untrack(() => memo());
  return <p>{value}</p>;
}

const shared = createRoot(() => createMemo(async () => 1));

// Not the body's own: `owner._parent !== context`, so no pending throw.
export function Elsewhere(props: { value: () => number }) {
  const one = untrack(() => shared());
  const two = untrack(() => props.value());
  return <p>{[one, two].length}</p>;
}

// A child root changes `context`, and a function that is no `untrack` proves nothing.
export function NotThisOwner() {
  const memo = createMemo(async () => 1);
  let one = 0;
  createRoot(() => {
    one = untrack(() => memo());
  });
  const peek = <T,>(fn: () => T) => fn;
  const two = peek(() => memo());
  return <p>{[one, two].length}</p>;
}
