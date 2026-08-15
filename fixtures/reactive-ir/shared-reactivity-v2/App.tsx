// The shared fine-grained rules under the 2.0 dialect. The defects here are
// defects in both language versions, so the 2.0 catalog carries them under
// the checker's plain names with the same SC codes as their v1/ twins (which
// fixtures/reactive-ir/v1-reactivity pins). This project pins the 2.0 exits.
import { createMemo, createSignal, createStore } from "solid-js";

// uncalled-accessor (SC1005): the accessor is interpolated uncalled, so the
// template renders the function source instead of the value.
export function Templated() {
  const [count] = createSignal(0);
  return <div>{`count is ${count}`}</div>;
}

// The corrected form: the accessor is called, the template sees the value,
// and the rule must stay silent.
export function TemplatedCalled() {
  const [count] = createSignal(0);
  return <div>{`count is ${count()}`}</div>;
}

// A memo accessor is an accessor too: interpolating it uncalled is the same
// defect, and proving it from the engine (not the name) is the point.
export function TemplatedMemo() {
  const [count] = createSignal(0);
  const doubled = createMemo(() => count() * 2);
  return <div>{`doubled is ${doubled}`}</div>;
}

// A native value attribute receives the function object eagerly. This is a
// second SC1005 position, independent of template-literal coercion.
export function NativeAttributeAccessor() {
  const [count] = createSignal(0);
  return <div title={count}>count</div>;
}

// no-direct-mutation (SC2003): a store is a readonly proxy in 2.0 exactly as
// in 1.x, so the write is dropped.
export function Mutates() {
  const [store] = createStore({ open: false });
  const toggle = () => {
    store.open = true;
  };
  return <button onClick={toggle}>{String(store.open)}</button>;
}

// The corrected form goes through the setter, inside the event handler 2.0
// allows to write, and must stay silent.
export function MutatesThroughSetter() {
  const [store, setStore] = createStore({ open: false });
  return <button onClick={() => setStore({ open: true })}>{String(store.open)}</button>;
}

// 2.0 write-enables the original proxy for the duration of its own setter's
// draft callback (probed on rc.0: the write commits), so this is correct
// code and must stay silent.
export function MutatesInsideOwnSetter() {
  const [store, setStore] = createStore({ open: false });
  return <button onClick={() => setStore(() => { store.open = true; })}>{String(store.open)}</button>;
}

// Another store's setter enables nothing for this proxy: the write is
// silently dropped at runtime and stays a finding.
export function MutatesInsideOtherSetter() {
  const [store] = createStore({ open: false });
  const [other, setOther] = createStore({ count: 0 });
  return <button onClick={() => setOther(() => { store.open = true; })}>{String(store.open)}{other.count}</button>;
}

// expected-function-got-expression (SC1007): count() runs during setup and
// its result -- a number, not a function -- is bound as the listener.
export function CalledHandler() {
  const [count] = createSignal(0);
  return <button onClick={count()}>{count()}</button>;
}

// The corrected form wraps the read in a handler function and must stay
// silent; so must a factory call, which returns the handler and is the false
// positive a syntax-only rule cannot avoid.
export function WrappedHandler() {
  const [count] = createSignal(0);
  return <button onClick={() => count()}>{count()}</button>;
}

export function FactoryHandler() {
  return <button onClick={makeHandler()}>ok</button>;
}

// Reading a handler through reactive props during native element setup
// freezes the initial function. This exercises SC1007's member-read branch,
// distinct from calling an accessor and binding its returned value.
export function ReactiveMemberHandler(props: { onSave: () => void }) {
  return <button onClick={props.onSave}>save</button>;
}

// untracked-derived-function (SC1006): doubled derives from count, and the
// only call to it is a plain statement in the component body, so the
// derivation reads once and subscribes to nothing.
export function DerivedButDiscarded() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  console.log(doubled());
  return <div>static</div>;
}

// Derivation is transitive: labelled reads doubled, which reads count. Only
// labelled is invoked in the untracked component body, so SC1006 must follow
// the dependency chain rather than relying on a direct signal read.
export function TransitivelyDerivedButDiscarded() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  const labelled = () => String(doubled());
  console.log(labelled());
  return <div>static</div>;
}

// Called from JSX, which tracks: the same shape, and not a defect.
export function DerivedAndRendered() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  return <div>{doubled()}</div>;
}

declare function makeHandler(): () => void;
