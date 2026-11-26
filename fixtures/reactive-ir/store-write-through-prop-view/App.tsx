import { createContext, createStore, merge, omit, useContext } from "solid-js";
import type { StoreSetter } from "solid-js";

// ADR 1211: ADR 1172's census of a handed prop's reads follows the props object through a `merge` or `omit` view held in a `const`,
// and through a whole-props spread onto a local component. A store handed with its setter, the setter never read and the store read
// only as the root of a contained use, still holds its own literal (ADR 0491): a write past the root is dropped outside the setter
// (STORE_WRITE_OUTSIDE_SETTER).

type Items = { a: { v: number }; items: number[] };
type Props = { s1: Items; setS1: StoreSetter<Items> };

// Violations.
function MergeChild(props: Props) {
  const merged = merge({ extra: 1 }, props);
  merged.s1.a.v = 7;
  return <i />;
}
export function MergeParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return <MergeChild s1={s1} setS1={setS1} />;
}

function OmitChild(props: Props & { extra: number }) {
  const rest = omit(props, "extra");
  rest.s1.a.v = 7;
  return <i />;
}
export function OmitParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return <OmitChild s1={s1} setS1={setS1} extra={1} />;
}

function ForwardChild(props: Props) {
  props.s1.a.v = 7;
  return <i />;
}
function ForwardMid(props: Props) {
  return <ForwardChild {...props} />;
}
export function ForwardParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return <ForwardMid s1={s1} setS1={setS1} />;
}

// An omit view that drops the store and its setter, spread onto an element: it reads neither.
function ViewSpreadChild(props: Props & { title?: string }) {
  const merged = merge({ extra: 1 }, props);
  const rest = omit(props, "s1", "setS1");
  merged.s1.items.push(1);
  return <p {...rest}>v</p>;
}
export function ViewSpreadParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return <ViewSpreadChild s1={s1} setS1={setS1} />;
}

// Uncertifiable, as before.
// The setter is read through the view and called: a link may hold a value the literal did not write.
function MergeCallsSetterChild(props: Props) {
  const merged = merge({ extra: 1 }, props);
  merged.s1.a.v = 7;
  return <button onClick={() => merged.setS1((draft) => { draft.a = { v: 1 }; })} />;
}
export function MergeCallsSetterParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return <MergeCallsSetterChild s1={s1} setS1={setS1} />;
}

// The view is handed on whole.
function consume(_value: unknown) {}
function MergeHandedChild(props: Props) {
  const merged = merge({ extra: 1 }, props);
  consume(merged);
  merged.s1.a.v = 7;
  return <i />;
}
export function MergeHandedParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return <MergeHandedChild s1={s1} setS1={setS1} />;
}

// Forwarded to a component that calls the setter.
function ForwardCallsChild(props: Props) {
  props.s1.a.v = 7;
  return <button onClick={() => props.setS1((draft) => { draft.a = { v: 1 }; })} />;
}
function ForwardCallsMid(props: Props) {
  return <ForwardCallsChild {...props} />;
}
export function ForwardCallsParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return <ForwardCallsMid s1={s1} setS1={setS1} />;
}

// ADR 1211 slice 2: the store and its setter handed in a provider's value literal of a context this file declares and never
// exports: the census reads the key through every `useContext` result (an identifier read by static keys, or a destructure).
const ValCtx = createContext<Props>();
function ContextChild() {
  const ctx = useContext(ValCtx);
  ctx.s1.a.v = 7;
  return <i />;
}
export function ContextParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return (
    <ValCtx value={{ s1, setS1 }}>
      <ContextChild />
    </ValCtx>
  );
}

const PairCtx = createContext<Props>();
function ContextDestructuredChild() {
  const { s1 } = useContext(PairCtx);
  s1.items.push(1);
  return <i />;
}
export function ContextDestructuredParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return (
    <PairCtx value={{ s1, setS1 }}>
      <ContextDestructuredChild />
    </PairCtx>
  );
}

// Uncertifiable, as before: the setter read through the context and called; the context's value handed on; the context exported.
const CallsCtx = createContext<Props>();
function ContextCallsSetterChild() {
  const ctx = useContext(CallsCtx);
  ctx.s1.a.v = 7;
  return <button onClick={() => ctx.setS1((draft) => { draft.a = { v: 1 }; })} />;
}
export function ContextCallsSetterParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return (
    <CallsCtx value={{ s1, setS1 }}>
      <ContextCallsSetterChild />
    </CallsCtx>
  );
}

const HandedCtx = createContext<Props>();
function ContextHandedChild() {
  const ctx = useContext(HandedCtx);
  consume(ctx);
  ctx.s1.a.v = 7;
  return <i />;
}
export function ContextHandedParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return (
    <HandedCtx value={{ s1, setS1 }}>
      <ContextHandedChild />
    </HandedCtx>
  );
}

export const SharedCtx = createContext<Props>();
function ContextExportedChild() {
  const ctx = useContext(SharedCtx);
  ctx.s1.a.v = 7;
  return <i />;
}
export function ContextExportedParent() {
  const [s1, setS1] = createStore({ a: { v: 0 }, items: [0] });
  return (
    <SharedCtx value={{ s1, setS1 }}>
      <ContextExportedChild />
    </SharedCtx>
  );
}
