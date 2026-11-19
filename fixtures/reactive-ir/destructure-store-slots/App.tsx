import { createMemo, createSignal, createStore } from "solid-js";

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
    </div>
  );
}
