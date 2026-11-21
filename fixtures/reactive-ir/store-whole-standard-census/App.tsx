import { createOptimisticStore, createStore, deep, snapshot } from "solid-js";
import * as S from "solid-js";

// ADR 1178: a store handed whole to a reviewed standard function or primitive, with the result discarded or inert. The read is
// the whole-store row's (ADRs 0620, 0621): in a click handler no window warns, so each is clean, census rows included.
export function HandlerForms() {
  const [row] = createStore({ a: { v: 1 } });
  const [list] = createStore([1, 2]);
  const click = () => {
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
    console.log(`${row}`.length);
  };
  return <button onClick={click} />;
}

// The same forms in the body: each warns STRICT_READ_UNTRACKED on rc.21 where its row says so (snapshot and hasOwn read none).
export function BodyForms() {
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
export function ResultUsed() {
  const [row] = createStore({ a: { v: 1 } });
  const [list] = createStore([{ v: 1 }]);
  const click = () => {
    console.log(Object.values(row)[0].v);
    const items = [...list];
    console.log(items[0].v);
    for (const item of list) console.log(item.v);
    console.log(snapshot(row).a.v);
  };
  return <button onClick={click} />;
}

// Not the reviewed forms: a shadowed `snapshot`, a pattern loop binding, a call spread, a `map` callback on the result.
export function NotTheForms() {
  const [row] = createStore({ a: { v: 1 } });
  const [list] = createStore([{ v: 1 }]);
  const local = <T,>(value: T) => value;
  const click = () => {
    local(row);
    for (const { v } of list) console.log(v);
    console.log(...list);
    Object.values(row).map((x) => x.v);
  };
  return <button onClick={click} />;
}

// A store that may not have settled, or whose backing is not its literal: `snapshot` and `hasOwn` stay uncertifiable.
export function Unsettled(props: { init: { a: number } }) {
  const [derived] = createStore((draft: { a: number }) => void draft, { a: 1 });
  const [optimistic] = createOptimisticStore({ a: 1 });
  const [given] = createStore(props.init);
  const click = () => {
    snapshot(derived);
    snapshot(optimistic);
    void Object.hasOwn(given, "a");
  };
  return <button onClick={click} />;
}
