import { createEffect, createSignal, createStore } from "solid-js";

// ADR 1106: SC1003 at a store destructure. A violation stands only where setup provably runs it (the component's body, or a
// layer that runs in place there); a destructure whose run the analysis cannot place is uncertifiable, never silent.

// Kept: in the body, a frozen snapshot (violation).
function InBody() {
  const [store] = createStore({ v: 0 });
  const { v } = store;
  return <i>{v}</i>;
}

// Kept: in a `forEach` callback in the body, which runs in place (violation).
function InPlaceLayer() {
  const [store] = createStore({ v: 0 });
  [1].forEach(() => {
    const { v } = store;
    void v;
  });
  return <i />;
}

// U (was silent): a named helper the body calls runs during setup, where Chrome reports STRICT_READ_UNTRACKED.
function NamedHelper() {
  const [store] = createStore({ v: 0 });
  const read = () => {
    const { v } = store;
    return v;
  };
  read();
  return <i />;
}

// U (was silent): an effect's apply may run with a strict-read window open.
function InApply() {
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

// U (was silent): a function nested in a named ref callback runs in the callback's own pass.
function NestedInRef() {
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

// U (was a violation): a listener's run is no setup; nothing places it (Chrome reports nothing when a handler fires it).
function InListener() {
  const [store] = createStore({ v: 0 });
  const el = document.createElement("button");
  el.addEventListener("click", () => {
    const { v } = store;
    void v;
  });
  return <button onClick={() => el.click()} />;
}

// U (was a violation): an object's method runs where something calls it.
function InMethod() {
  const [store] = createStore({ v: 0 });
  const box = {
    handle: () => {
      const { v } = store;
      void v;
    },
  };
  window.addEventListener("click", () => box.handle());
  return <i />;
}

// Kept silent: a helper only a click handler calls reads fresh values.
function HandlerHelper() {
  const [store] = createStore({ v: 0 });
  const read = () => {
    const { v } = store;
    return v;
  };
  return <button onClick={() => read()} />;
}

export function Page() {
  const [count] = createSignal(0);
  return (
    <div>
      {count()}
      <InBody />
      <InPlaceLayer />
      <NamedHelper />
      <InApply />
      <NestedInRef />
      <InListener />
      <InMethod />
      <HandlerHelper />
    </div>
  );
}
