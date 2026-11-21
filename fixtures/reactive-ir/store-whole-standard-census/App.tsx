import { createMemo, createOptimisticStore, createStore, deep, snapshot } from "solid-js";
import * as S from "solid-js";

// ADR 1178: a store handed whole to a reviewed standard function or primitive, with the result discarded or inert. The read is
// the whole-store row's (ADRs 0620, 0621), tracked in a memo: clean, census rows included.
function TrackedForms() {
  const [row] = createStore({ a: { v: 1 } });
  const [list] = createStore([1, 2]);
  const total = createMemo(() => {
    Object.values(row);
    Object.entries(row);
    Array.from(list);
    void Object.hasOwn(row, "a");
    snapshot(row);
    deep(row);
    S.snapshot(row);
    void [...list];
    void { ...row };
    for (const item of list) {
      void item;
    }
    return `${row}`.length;
  });
  return <p>{total()}</p>;
}

// The same forms in the body: each warns STRICT_READ_UNTRACKED on rc.21 where its row says so (snapshot and hasOwn read none).
function BodyForms() {
  const [row] = createStore({ a: { v: 1 } });
  const [list] = createStore([1, 2]);
  Object.values(row);
  Array.from(list);
  void [...list];
  void Object.hasOwn(row, "a");
  snapshot(row);
  return <p />;
}

// The result held and read: an element may be a nested store, whose Get has no row. Uncertifiable, never clean.
function ResultUsed() {
  const [row] = createStore({ a: { v: 1 } });
  const [list] = createStore([{ v: 1 }]);
  return (
    <button
      onClick={() => {
        console.log(Object.values(row)[0].v);
        const items = [...list];
        console.log(items[0].v);
        for (const item of list) console.log(item.v);
        console.log(snapshot(row).a.v);
      }}
    />
  );
}

// Not the reviewed forms: a shadowed `snapshot`, a pattern loop binding, a call spread, a `map` callback on the result.
function NotTheForms() {
  const [row] = createStore({ a: { v: 1 } });
  const [list] = createStore([{ v: 1 }]);
  const local = <T,>(value: T) => value;
  return (
    <button
      onClick={() => {
        local(row);
        for (const { v } of list) console.log(v);
        console.log(...list);
        Object.values(row).map((x) => x.v);
      }}
    />
  );
}

// A store that may not have settled, or whose backing is not its literal: `snapshot` and `hasOwn` stay uncertifiable.
const initial: { a: number } = { a: 1 };

function Unsettled() {
  const [derived] = createStore((draft: { a: number }) => void draft, { a: 1 });
  const [optimistic] = createOptimisticStore({ a: 1 });
  const [given] = createStore(initial);
  return (
    <button
      onClick={() => {
        snapshot(derived);
        snapshot(optimistic);
        void Object.hasOwn(given, "a");
      }}
    />
  );
}

export function Page() {
  return (
    <>
      <TrackedForms />
      <BodyForms />
      <ResultUsed />
      <NotTheForms />
      <Unsettled />
    </>
  );
}
