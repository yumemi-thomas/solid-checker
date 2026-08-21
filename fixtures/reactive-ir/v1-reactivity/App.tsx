import { createEffect, createMemo, createSignal } from "solid-js";
import { createStore } from "solid-js/store";
import { described, observe } from "uncharted-helpers";

// v1/uncalled-accessor: the accessor is interpolated, so the template renders
// the function source instead of the value.
export function Templated() {
  const [count] = createSignal(0);
  return <div>{`count is ${count}`}</div>;
}

// A memo accessor is an accessor: passing it on is correct, and upstream
// reports it while leaving the structurally identical signal alone.
// https://github.com/solidjs-community/eslint-plugin-solid/issues/182
export function PassedOn() {
  const [count] = createSignal(0);
  const doubled = createMemo(() => count() * 2);
  return <div>{consume(count)}{consume(doubled)}</div>;
}

function consume(source: () => number) {
  return source();
}

// v1/no-direct-mutation: a store is a readonly proxy, so the write is dropped.
export function Mutates() {
  const [store] = createStore({ open: false });
  const toggle = () => {
    store.open = true;
  };
  return <button onClick={toggle}>{String(store.open)}</button>;
}

// Read-modify-write forms drop the write through the same readonly proxy.
// Upstream's rule sees the compound form (an ESTree AssignmentExpression) and
// reports it, but a member `++` is an UpdateExpression its props branch never
// looks at; no upstream case covers either spelling. The checker reports both,
// because what reaches the proxy is identical.
export function MutatesInPlace() {
  const [store] = createStore({ count: 0 });
  const bump = () => {
    store.count += 1;
    store.count++;
  };
  return <button onClick={bump}>{String(store.count)}</button>;
}

// v1/no-async-tracked-scope: tracking stops at the first await, so theme() is
// never a dependency and the effect stops responding to it.
export function AsyncEffect() {
  const [theme] = createSignal("dark");
  createEffect(async () => {
    await load();
    apply(theme());
  });
}

// The same await inside a synchronous scope is not this rule's business, and
// the 1.x seed argument is a value rather than a second callback: reading slot
// 1 as a tracked scope is how a 2.0-shaped table misreports this call.
export function SyncEffect() {
  const [theme] = createSignal("dark");
  createEffect((previous?: string) => {
    apply(theme());
    return previous;
  }, "light");
}

// v1/reactive-handler-frozen:          count() runs during setup and its
// result is bound as the listener.
export function CalledHandler() {
  const [count] = createSignal(0);
  return <button onClick={count()}>{count()}</button>;
}

// A factory call in the same position is correct — it returns the handler —
// and is the false positive a syntax-only rule cannot avoid reporting.
export function FactoryHandler() {
  return <button onClick={makeHandler()}>ok</button>;
}

declare const maybeUncheckedHandler: (() => void) | number;

// TypeScript deliberately skips hyphenated JSX attribute names, but the 1.x
// compiler still lowers every native `on` prefix as an event. The number is a
// proven SC1007 violation; the union remains an explicit uncertifiable shape.
export function UncheckedHandlers() {
  const numberHandler = 1;
  return (
    <div>
      <button on-event={numberHandler}>invalid</button>
      <button on-maybe={maybeUncheckedHandler}>uncertain</button>
      <button on-safe={() => {}}>safe</button>
    </div>
  );
}

// v1/reactive-source-uncaptured: the contract describes `described` and says
// its first argument is tracked, so that call is certified. It says nothing
// about `observe`, which has no body here either — so whether the accessor
// stays reactive through it is unknowable, and the call site says so.
export function Uncaptured() {
  const [count] = createSignal(0);
  described(count);
  observe(count);
}

// v1/strict-read-untracked: the untracked call reaches count through doubled
// so SC1001 follows the helper-call chain and anchors at the invocation.
export function DerivedButDiscarded() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  console.log(doubled());
  return <div>static</div>;
}

// Called from JSX, which tracks. The same shape, and not a defect — reporting
// this is what a rule that guesses at the negative would do.
export function DerivedAndRendered() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  return <div>{doubled()}</div>;
}

// Passed rather than called: no read happens in this component body, so there
// is no untracked-read finding to emit here in this component.
export function DerivedAndPassed() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  described(doubled);
  return <div>static</div>;
}

// Called inside a tracked callback. The call is in a nested function, so it is
// unreachable from a lexical scan of the component body alone.
export function DerivedInEffect() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  createEffect(() => {
    apply(String(doubled()));
  });
  return <div>static</div>;
}

// Derives from nothing reactive, so there is no reactivity to lose.
export function PlainHelper() {
  const twice = () => 2 * 2;
  console.log(twice());
  return <div>static</div>;
}

// Rendered through a fragment, which tracks its children exactly as an
// element does. Only the element table answered "is this call inside JSX"
// at first, so this correct pattern was reported as never tracked.
export function DerivedInFragment() {
  const [count] = createSignal(0);
  const doubled = () => count() * 2;
  return <>{doubled()}</>;
}

// Ambient callees come from no package, so no contract could ever describe
// them; reporting these demanded a fix nobody can apply, and the rule now
// reports only callees imported from a package (like `observe` above).
export function AmbientCallees() {
  const [count] = createSignal(0);
  setTimeout(count, 100);
  console.log(count);
  return <div>{count()}</div>;
}

// v1/reactive-dispatch-unresolved: the interface is type-correct for both
// values, but only one method reads a signal. Neither candidate may be chosen
// as the truth, so the call remains an explicit proof obligation.
export function ConditionalMethodDispatch() {
  const [count] = createSignal(0);
  const reactive = { read: () => count() };
  const quiet = { read: () => 0 };
  const invoke = (reader: { read(): number }) => reader.read();
  const value = invoke(Math.random() > 0.5 ? reactive : quiet);
  return <div>{value}</div>;
}

declare function load(): Promise<void>;
declare function apply(theme: string): void;
declare function makeHandler(): () => void;
