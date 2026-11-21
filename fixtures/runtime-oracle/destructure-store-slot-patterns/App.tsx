import * as S from "solid-js";
import { createEffect, createSignal, createStore } from "solid-js";

// ADR 1162: a store destructure SC1003 hands to SC1001's placement reads each property as the Get of its key, whatever the
// property's value: a quoted or renamed key, a default, or an empty nested pattern. In an effect's apply the strict-read window
// is open: each warns (STRICT_READ_UNTRACKED), unless its key is `then` and the backing runs nothing.

// Violations: each property's key warns in the apply.
function QuotedInApply() {
  const [store] = createStore({ v: 0 });
  createEffect(() => 1, () => {
    const { "v": x } = store;
    void x;
  });
  return <i />;
}

function EmptySubInApply() {
  const [store] = createStore({ a: { b: 1 } });
  createEffect(() => 1, () => {
    const { a: {} } = store;
  });
  return <i />;
}

function EmptyItemsInApply() {
  const [store] = createStore({ items: [1] });
  createEffect(() => 1, () => {
    const { items: [] } = store;
  });
  return <i />;
}

function EmptySiblingInApply() {
  const [store] = createStore({ v: 0, a: { b: 1 } });
  createEffect(() => 1, () => {
    const { v, a: {} } = store;
    void v;
  });
  return <i />;
}

function RenamedDefaultInApply() {
  const [store] = createStore({ v: 0 } as { v?: number });
  createEffect(() => 1, () => {
    const { v: w = 1 } = store;
    void w;
  });
  return <i />;
}

// The namespace import's store and effect: the same Gets.
function NamespaceQuotedInApply() {
  const [store] = S.createStore({ v: 0 });
  S.createEffect(() => 1, () => {
    const { 'v': x } = store;
    void x;
  });
  return <i />;
}

// Clean: a `then` Get does not warn, and the literal's `then` is data nothing writes (the setter is never referenced).
function ThenRenamedInApply() {
  const [store] = createStore({ then: 0, v: 0 });
  createEffect(() => 1, () => {
    const { then: w } = store;
    void w;
  });
  return <i />;
}

function ThenQuotedInApply() {
  const [store] = createStore({ then: 0, v: 0 });
  createEffect(() => 1, () => {
    const { "then": w } = store;
    void w;
  });
  return <i />;
}

function ThenDefaultInApply() {
  const [store] = createStore({ then: 0, v: 0 });
  createEffect(() => 1, () => {
    const { then = 1 } = store;
    void then;
  });
  return <i />;
}

function ThenNestedEmptyInApply() {
  const [store] = createStore({ then: 0, v: 0 });
  createEffect(() => 1, () => {
    const { then: {} } = store;
  });
  return <i />;
}

// A handler: the slots take their member twin's verdict (`store.v` there is uncertifiable `function-run-unplaced` too).
function QuotedInHandler() {
  const [store] = createStore({ v: 0, a: { b: 1 } });
  return (
    <button onClick={() => {
      const { "v": x, a: {} } = store;
      void x;
    }}>go</button>
  );
}

// Uncertifiable, as before.
// The setter is referenced: the backing's `then` may be written with a getter, which a `then` Get would run.
function ThenWrittenBacking() {
  const [store, setStore] = createStore({ then: 0, v: 0 });
  createEffect(() => 1, () => {
    const { then: w } = store;
    void w;
  });
  return <button onClick={() => setStore((draft) => { draft.v = 1; })}>go</button>;
}

// A non-empty nested pattern reads past its key: the chained Get under `then` is not handed over.
function ThenNestedInApply() {
  const [store] = createStore({ then: { b: 0 }, v: 0 });
  createEffect(() => 1, () => {
    const { then: { b } } = store;
    void b;
  });
  return <i />;
}

// A computed key no fact names.
function ComputedKeyInApply() {
  const [store] = createStore({ then: 0, v: 0 });
  const key: "then" | "v" = "then";
  createEffect(() => 1, () => {
    const { [key]: w } = store;
    void w;
  });
  return <i />;
}

export function Page() {
  const [count] = createSignal(0);
  return (
    <div>
      {count()}
      <QuotedInApply />
      <EmptySubInApply />
      <EmptyItemsInApply />
      <EmptySiblingInApply />
      <RenamedDefaultInApply />
      <NamespaceQuotedInApply />
      <ThenRenamedInApply />
      <ThenQuotedInApply />
      <ThenDefaultInApply />
      <ThenNestedEmptyInApply />
      <QuotedInHandler />
      <ThenWrittenBacking />
      <ThenNestedInApply />
      <ComputedKeyInApply />
    </div>
  );
}
