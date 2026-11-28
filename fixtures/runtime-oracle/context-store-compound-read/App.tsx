import { createContext, createStore, useContext } from "solid-js";
import type { StoreSetter } from "solid-js";

// ADR 1236: a store and its setter handed in a provider's value literal (ADR 1211) and written through a `useContext` result. A write that
// reads its target first (`+=`, `++`, `||=`) Gets the member before it Sets it: rc.21 warns STRICT_READ_UNTRACKED at that Get as well as
// STORE_WRITE_OUTSIDE_SETTER at the write. The store write's SC2003 row does not answer the Get, so the value-use census keeps its
// SC9012 row there (`value-held-in-binding`): uncertifiable, where it was certified clean. A plain write (`=`) Gets nothing.

type Props = { s1: { a: { v: number }; items: number[] }; setS1: StoreSetter<{ a: { v: number }; items: number[] }> };

const CompoundCtx = createContext<Props>();
function CompoundChild() {
  const ctx = useContext(CompoundCtx);
  ctx.s1.a.v += 1;
  return <i />;
}
export function CompoundParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return (
    <CompoundCtx value={{ s1, setS1 }}>
      <CompoundChild />
    </CompoundCtx>
  );
}

const IncrementCtx = createContext<Props>();
function IncrementChild() {
  const ctx = useContext(IncrementCtx);
  ctx.s1.a.v++;
  return <i />;
}
export function IncrementParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return (
    <IncrementCtx value={{ s1, setS1 }}>
      <IncrementChild />
    </IncrementCtx>
  );
}

const LogicalCtx = createContext<Props>();
function LogicalChild() {
  const ctx = useContext(LogicalCtx);
  ctx.s1.a.v ||= 1;
  return <i />;
}
export function LogicalParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return (
    <LogicalCtx value={{ s1, setS1 }}>
      <LogicalChild />
    </LogicalCtx>
  );
}

const PlainCtx = createContext<Props>();
function PlainChild() {
  const ctx = useContext(PlainCtx);
  ctx.s1.a.v = 7;
  return <i />;
}
export function PlainParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return (
    <PlainCtx value={{ s1, setS1 }}>
      <PlainChild />
    </PlainCtx>
  );
}

// The destructured reader.
const PairCtx = createContext<Props>();
function DestructuredChild() {
  const { s1 } = useContext(PairCtx);
  s1.a.v += 1;
  return <i />;
}
export function DestructuredParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return (
    <PairCtx value={{ s1, setS1 }}>
      <DestructuredChild />
    </PairCtx>
  );
}
