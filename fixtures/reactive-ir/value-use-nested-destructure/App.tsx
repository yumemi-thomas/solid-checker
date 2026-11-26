import { createMemo, createSignal, createStore } from "solid-js";

// Negative: a pattern nested in a const declarator's pattern takes the accessor at its key; the call in the memo is tracked.
export function NestedMemo() {
  const [count] = createSignal(0);
  const { a: { useIt } } = { a: { useIt: count } };
  const m = createMemo(() => useIt());
  return <p>{m()}</p>;
}

// Negative: the same with a store, read in the memo.
export function NestedStoreMemo() {
  const [s] = createStore({ v: 0 });
  const { a: { st } } = { a: { st: s } };
  const m = createMemo(() => st.v);
  return <p>{m()}</p>;
}

// Positive: the accessor taken the same way, called in the body (untracked): a row at the call.
export function NestedBody() {
  const [count] = createSignal(0);
  const { a: { useIt } } = { a: { useIt: count } };
  void useIt();
  return <p>x</p>;
}

// Controls, still the carrier's rows: a spread in the literal, a repeated key, a rest in the pattern, a `let`, two levels of nesting.
export function SpreadLiteral(props: { o: object }) {
  const [count] = createSignal(0);
  const { a: { useIt } } = { ...props.o, a: { useIt: count } };
  const m = createMemo(() => useIt());
  return <p>{m()}</p>;
}

export function RepeatedKey() {
  const [count] = createSignal(0);
  const other = { useIt: () => 1 };
  const k = "a" as string;
  const { a: { useIt } } = { a: { useIt: count }, [k]: other };
  const m = createMemo(() => useIt());
  return <p>{m()}</p>;
}

export function RestPattern() {
  const [count] = createSignal(0);
  const { a: { useIt }, ...rest } = { a: { useIt: count }, b: 1 };
  void rest;
  const m = createMemo(() => useIt());
  return <p>{m()}</p>;
}

export function LetDeclarator() {
  const [count] = createSignal(0);
  let { a: { useIt } } = { a: { useIt: count } };
  const m = createMemo(() => useIt());
  useIt = () => 1;
  return <p>{m()}</p>;
}

export function TwoLevels() {
  const [count] = createSignal(0);
  const { a: { b: { useIt } } } = { a: { b: { useIt: count } } };
  const m = createMemo(() => useIt());
  return <p>{m()}</p>;
}
