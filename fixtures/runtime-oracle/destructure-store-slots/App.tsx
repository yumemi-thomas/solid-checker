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
