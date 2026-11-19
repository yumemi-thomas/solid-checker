import { createStore } from "solid-js";

type S = { then: number; v: number };
const K = "then";

// ADR 1122: a store destructure in a component body, decided by its properties' keys by value. A `then` Get does not warn,
// whatever the spelling, default or nested value; a pattern with no property reads nothing.
export function ThenDefault() {
  const [store] = createStore<S>({ then: 1, v: 0 });
  const { then = 1 } = store;
  void then;
  return <p />;
}

export function ThenRenamedDefault() {
  const [store] = createStore<S>({ then: 1, v: 0 });
  const { then: w = 1 } = store;
  void w;
  return <p />;
}

export function ThenNestedEmpty() {
  const [store] = createStore<S>({ then: 1, v: 0 });
  const { then: {} } = store;
  return <p />;
}

export function ThenEscaped() {
  const [store] = createStore<S>({ then: 1, v: 0 });
  const { then } = store;
  void then;
  return <p />;
}

export function ThenRenamed() {
  const [store] = createStore<S>({ then: 1, v: 0 });
  const { then: w } = store;
  void w;
  return <p />;
}

export function ThenQuoted() {
  const [store] = createStore<S>({ then: 1, v: 0 });
  const { "then": w } = store;
  void w;
  return <p />;
}

export function ThenComputedComment() {
  const [store] = createStore<S>({ then: 1, v: 0 });
  const { /* c */ [K]: w } = store;
  void w;
  return <p />;
}

export function EmptyComment() {
  const [store] = createStore<S>({ then: 1, v: 0 });
  const { /* nothing */ } = store;
  return <p />;
}

export function EmptyLineComment() {
  const [store] = createStore<S>({ then: 1, v: 0 });
  const {
    // nothing
  } = store;
  return <p />;
}

// Violations kept: another key is read.
export function ThenAndV() {
  const [store] = createStore<S>({ then: 1, v: 0 });
  const { then, v } = store;
  void then;
  void v;
  return <p />;
}

export function CommentAndV() {
  const [store] = createStore<S>({ then: 1, v: 0 });
  const { /* c */ v } = store;
  void v;
  return <p />;
}

export function ComputedLiteralV() {
  const [store] = createStore<S>({ then: 1, v: 0 });
  const { ["v"]: x } = store;
  void x;
  return <p />;
}

export function Page() {
  return (
    <div>
      <ThenDefault />
      <ThenRenamedDefault />
      <ThenNestedEmpty />
      <ThenEscaped />
      <ThenRenamed />
      <ThenQuoted />
      <ThenComputedComment />
      <EmptyComment />
      <EmptyLineComment />
      <ThenAndV />
      <CommentAndV />
      <ComputedLiteralV />
    </div>
  );
}
