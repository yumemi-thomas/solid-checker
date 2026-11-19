import { createEffect, createMemo, createSignal, createStore } from "solid-js";

// ADR 1107: SC1003's row at a store destructure takes the verdict SC1001 gives its slots read as members (`const { v } = s`
// is `s.v`), so the row stands where SC1001's placement does.

// A function handed by name to a primitive and also called by the body (the review's F1): the body's call reads in setup.
function HandedAndCalled() {
  const [store] = createStore({ v: 0 });
  const read = () => {
    const { v } = store;
    return v;
  };
  createMemo(read);
  read();
  return <i />;
}

// A slot with a default is still read.
function SlotWithDefault() {
  const [store] = createStore({ v: 0 } as { v?: number });
  const { v = 1 } = store;
  return <i>{v}</i>;
}

// A rest element reads the whole object.
function RestElement() {
  const [store] = createStore({ v: 0, w: 1 });
  const { v, ...rest } = store;
  return <i>{v}{rest.w}</i>;
}

// An empty pattern reads nothing: no row.
function EmptyPattern() {
  const [store] = createStore({ v: 0 });
  const {} = store;
  return <i />;
}

// A computed key: SC1001's placement needs a static name, so the row is uncertifiable.
function ComputedKey() {
  const [store] = createStore({ v: 0 });
  const key = "v" as const;
  const { [key]: v } = store;
  return <i>{v}</i>;
}

// Round 2 (Fable review, F1): a sub-pattern that binds no name still reads its property.
function EmptySubPattern() {
  const [store] = createStore({ a: { b: 1 }, items: [1] });
  const { a: {} } = store;
  const { items: [] } = store;
  return <i />;
}

// Round 2 (F1): a sibling sub-pattern is not handed over with the named slot (the counts matched by coincidence).
function SiblingEmptyInHelper() {
  const [store] = createStore({ v: 0, a: { b: 1 } });
  const read = () => {
    const { v, a: {} } = store;
    return v;
  };
  read();
  return <i />;
}

// Round 2 (P1): a slot after an undecided `then` is read too.
function ThenFirstInApply() {
  const [store] = createStore({ then: 1, v: 0 });
  createEffect(
    () => 1,
    () => {
      const { then, v } = store;
      void then;
      void v;
    },
  );
  return <i />;
}

// Round 2 (P2): a quoted key is a decided key.
function QuotedKey() {
  const [store] = createStore({ v: 0 });
  const { "v": x } = store;
  return <i>{x}</i>;
}

export function Page() {
  const [count] = createSignal(0);
  return (
    <div>
      {count()}
      <HandedAndCalled />
      <SlotWithDefault />
      <RestElement />
      <EmptyPattern />
      <ComputedKey />
      <EmptySubPattern />
      <SiblingEmptyInHelper />
      <ThenFirstInApply />
      <QuotedKey />
    </div>
  );
}
