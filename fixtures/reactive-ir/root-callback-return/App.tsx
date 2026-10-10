import { createRoot, createSignal, createMemo } from "solid-js";
import * as Solid from "solid-js";
import { makeAccessor, makeTuple, makeObject, makePromise, makeChoice } from "reactive-package";

// Models the two timer ledger shapes and pagination's [, page] shape.
const now = createRoot(() => makeAccessor());
const count = createRoot(function () { return makeAccessor(); });
const [, page] = createRoot(() => makeTuple());
const { read: objectRead } = createRoot(() => makeObject());
const [signal, setSignal] = Solid.createRoot(() => Solid.createSignal(0));
const memo = createRoot(() => createMemo(() => 0));
const selected = createRoot(() => {
  const [read] = createSignal(0);
  return read;
});
const selectedObject = createRoot(() => {
  const { read } = makeObject();
  return read;
});
const wrapped = createRoot((() => (makeAccessor() as () => number)));

export function TimerRead() { const value = now(); return <div>{value}</div>; }
export function CounterRead() { const value = count(); return <div>{value}</div>; }
export function PaginationRead() { const value = page(); return <div>{value}</div>; }
export function ObjectRead() { const value = objectRead(); return <div>{value}</div>; }
export function SignalRead() { const value = signal(); return <div>{value}</div>; }
export function MemoRead() { const value = memo(); return <div>{value}</div>; }
export function SelectedRead() { const value = selected(); return <div>{value}</div>; }
export function SelectedObjectRead() { const value = selectedObject(); return <div>{value}</div>; }
export function WrappedRead() { const value = wrapped(); return <div>{value}</div>; }
export function TrackedReads() {
  return <div onClick={() => setSignal(1)}>{now() + count() + page() + objectRead() + signal() + memo() + selected() + selectedObject() + wrapped()}</div>;
}

declare function condition(): boolean;
declare function retain(value: unknown): void;
declare function unknownFactory(): () => number;

const namedInit = () => makeAccessor();
const named = createRoot(namedInit);
const multiple = createRoot(() => {
  if (condition()) return makeAccessor();
  return makeAccessor();
});
const conditional = createRoot(() => condition() ? makeAccessor() : makeAccessor());
const conditionalStatement = createRoot(() => {
  if (condition()) return makeAccessor();
  throw new Error("no return");
});
const disposeParameter = createRoot(dispose => { void dispose; return makeAccessor(); });
const disposal = createRoot(dispose => { const value = makeAccessor(); dispose(); return value; });
const disposeResult = createRoot(dispose => dispose);
const asyncResult = createRoot(async () => makeAccessor());
const unknown = createRoot(() => unknownFactory());
const promised = createRoot(() => makePromise());
const choice = createRoot(() => makeChoice());
const namedExpression = createRoot(function initializer() {
  retain(initializer);
  return makeAccessor();
});
const escaped = createRoot(() => makeAccessor());
retain([escaped]);
const shorthand = createRoot(() => makeAccessor());
retain({ shorthand });
const alias = escaped;
const localEscape = createRoot(() => {
  const value = makeAccessor();
  retain([value]);
  return value;
});
let mutable = createRoot(() => makeAccessor());
mutable = () => 0;
const [defaulted = () => 0] = createRoot(() => makeTuple());
const [...restItems] = createRoot(() => makeTuple());
retain(restItems);
const rest = createRoot(() => makeTuple());
const [getter] = rest;
const signalInit = () => createSignal(0);
const tupleContainer = createRoot(signalInit);
const memberGetter = tupleContainer[0];
const [tupleGetter] = tupleContainer;

// These calls must not become proven strict reads from their annotations.
export function UnresolvedReads() {
  const value = named() + multiple() + conditional() + conditionalStatement()
    + disposeParameter() + disposal() + unknown() + namedExpression()
    + escaped() + shorthand() + alias() + localEscape() + mutable()
    + defaulted() + getter() + memberGetter() + tupleGetter() + (choice?.() ?? 0);
  disposeResult();
  retain(asyncResult);
  retain(promised);
  return <div>{value}</div>;
}

// Exact binder resolution must prevent the local createRoot from receiving
// the dialect's passthrough fact.
const createPlain = (() => {
  const createRoot = (_init: () => () => number) => () => 7;
  return createRoot(() => makeAccessor());
})();
export function ShadowedRoot() { const value = createPlain(); return <div>{value}</div>; }

// No name-only matching of a callback local with the returned declaration.
const shadowedBinding = createRoot(() => {
  const read = () => 7;
  { const read = makeAccessor(); void read; }
  return read;
});
export function ShadowedBinding() { const value = shadowedBinding(); return <div>{value}</div>; }
