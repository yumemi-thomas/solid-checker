import { createContext, createStore, useContext } from "solid-js";
import type { StoreSetter } from "solid-js";

// ADR 1231: a handed store's read carried through a local carrier that hands it on unchanged (a closed literal's slot, an array
// element, a local hook's one return, an exact `const` binding, a destructuring, a static member read) is still a contained use: the
// store keeps its own literal (ADR 0491), and a write past its root is dropped outside the setter (STORE_WRITE_OUTSIDE_SETTER).

type Items = { a: { v: number }; items: number[] };
type Props = { s1: Items; setS1: StoreSetter<Items> };
type Vals = { s1: Items; setS1: StoreSetter<Items> };

// Violations.
function HookChild(props: Props) {
  const useThing = () => ({ useIt: props.s1 });
  const { useIt } = useThing();
  useIt.a.v = 7;
  return <i />;
}
export function HookParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return <HookChild s1={s1} setS1={setS1} />;
}

function TupleChild(props: Props) {
  const pair = [0, props.s1] as const;
  const [, useIt] = pair;
  useIt.a.v = 7;
  return <i />;
}
export function TupleParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return <TupleChild s1={s1} setS1={setS1} />;
}

const MemberCtx = createContext<Vals>();
function HookMemberChild() {
  const ctx = useContext(MemberCtx);
  const useThing = () => ({ useIt: ctx.s1 });
  const thing = useThing();
  thing.useIt.a.v = 7;
  return <i />;
}
export function HookMemberParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return (
    <MemberCtx value={{ s1, setS1 }}>
      <HookMemberChild />
    </MemberCtx>
  );
}

// Uncertifiable: the carrier goes where this does not follow.
const kept: unknown[] = [];
function keep(value: unknown): void {
  kept.push(value);
}

function KeptChild(props: Props) {
  const pair = [0, props.s1] as const;
  keep(pair);
  const [, useIt] = pair;
  useIt.a.v = 7;
  return <i />;
}
export function KeptParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return <KeptChild s1={s1} setS1={setS1} />;
}

function HandedHookChild(props: Props) {
  const useThing = () => ({ useIt: props.s1 });
  keep(useThing);
  const { useIt } = useThing();
  useIt.a.v = 7;
  return <i />;
}
export function HandedHookParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return <HandedHookChild s1={s1} setS1={setS1} />;
}

function DefaultChild(props: Props) {
  const useThing = () => ({ useIt: props.s1 });
  const { useIt = props.s1 } = useThing();
  useIt.a.v = 7;
  return <i />;
}
export function DefaultParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return <DefaultChild s1={s1} setS1={setS1} />;
}
