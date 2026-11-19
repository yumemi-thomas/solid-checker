import { createEffect, createMemo, createStore } from "solid-js";

// ADR 1107: Chrome reports STRICT_READ_UNTRACKED at each destructure below, where SC1003 now stands as a violation (or, where
// the analysis cannot place it, an obligation).
export function InApply() {
  const [store] = createStore({ v: 0 });
  createEffect(
    () => 1,
    () => {
      const { v } = store;
      void v;
    },
  );
  return <i />;
}
export function NestedInRef() {
  const [store] = createStore({ v: 0 });
  const attach = () => {
    const inner = () => {
      const { v } = store;
      void v;
    };
    inner();
  };
  return <p ref={attach} />;
}
export function HandedAndCalled() {
  const [store] = createStore({ v: 0 });
  const read = () => {
    const { v } = store;
    return v;
  };
  createMemo(read);
  read();
  return <i />;
}
export function SlotWithDefault() {
  const [store] = createStore({ v: 0 } as { v?: number });
  const { v = 1 } = store;
  return <i>{v}</i>;
}
export function RestElement() {
  const [store] = createStore({ v: 0, w: 1 });
  const { v, ...rest } = store;
  return <i>{v}{rest.w}</i>;
}

// Round 2 (Fable review, F1): a sub-pattern that binds no name still reads its property.
export function EmptySubPattern() {
  const [store] = createStore({ a: { b: 1 }, items: [1] });
  const { a: {} } = store;
  const { items: [] } = store;
  return <i />;
}

// Round 2 (F1): a sibling sub-pattern is not handed over with the named slot (the counts matched by coincidence).
export function SiblingEmptyInHelper() {
  const [store] = createStore({ v: 0, a: { b: 1 } });
  const read = () => {
    const { v, a: {} } = store;
    return v;
  };
  read();
  return <i />;
}

// Round 2 (P1): a slot after an undecided `then` is read too.
export function ThenFirstInApply() {
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
export function QuotedKey() {
  const [store] = createStore({ v: 0 });
  const { "v": x } = store;
  return <i>{x}</i>;
}
