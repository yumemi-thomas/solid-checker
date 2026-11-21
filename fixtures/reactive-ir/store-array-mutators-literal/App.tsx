import * as S from "solid-js";
import { createStore } from "solid-js";

// ADR 1170: `sort`, `reverse`, `fill` and `copyWithin` on a store path write an element when the store's own literal (ADR
// 0491: the setter is never referenced and no proxy leaves) proves the call reaches one. Every element write meets the read
// proxy's `set` trap outside the setter: STORE_WRITE_OUTSIDE_SETTER.

// Violations: the literal proves a write.
function SortNumbers() {
  const [store] = createStore({ items: [3, 1, 2] });
  store.items.sort();
  return <i />;
}

function SortComparator() {
  const [store] = createStore({ items: [1, 2] });
  store.items.sort((a, b) => b - a);
  return <i />;
}

function ReverseTwo() {
  const [store] = createStore({ items: [1, 2] });
  store.items.reverse();
  return <i />;
}

function FillOne() {
  const [store] = createStore({ items: [0] });
  store.items.fill(7);
  return <i />;
}

function FillRange() {
  const [store] = createStore({ items: [0, 0, 0] });
  store.items.fill(7, 1, 2);
  return <i />;
}

function CopyWithinTwo() {
  const [store] = createStore({ items: [1, 2] });
  store.items.copyWithin(0, 1);
  return <i />;
}

function NamespaceReverse() {
  const [store] = S.createStore({ items: [1, 2] });
  store.items.reverse();
  return <i />;
}

// Uncertifiable, as before: the call may write nothing (Chrome silent), or nothing proves it writes.
function ReverseOne() {
  const [store] = createStore({ items: [1] });
  store.items.reverse();
  return <i />;
}

function SortOne() {
  const [store] = createStore({ items: [1] });
  store.items.sort();
  return <i />;
}

function FillEmptyRange() {
  const [store] = createStore({ items: [0, 0] });
  store.items.fill(7, 1, 1);
  return <i />;
}

function CopyWithinPastEnd() {
  const [store] = createStore({ items: [1, 2] });
  store.items.copyWithin(0, 5);
  return <i />;
}

function SortStrings() {
  const [store] = createStore({ items: ["b", "a"] });
  store.items.sort();
  return <i />;
}

function SortOtherComparator() {
  const [store] = createStore({ items: [1, 2] });
  store.items.sort((a, b) => (a > b ? 1 : -1));
  return <i />;
}

function SetterReferenced() {
  const [store, setStore] = createStore({ items: [1, 2] });
  store.items.reverse();
  return <button onClick={() => setStore((draft) => { draft.items = []; })}>go</button>;
}

export function Page() {
  return (
    <div>
      <SortNumbers />
      <SortComparator />
      <ReverseTwo />
      <FillOne />
      <FillRange />
      <CopyWithinTwo />
      <NamespaceReverse />
      <ReverseOne />
      <SortOne />
      <FillEmptyRange />
      <CopyWithinPastEnd />
      <SortStrings />
      <SortOtherComparator />
      <SetterReferenced />
    </div>
  );
}
