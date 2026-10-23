import {
  createMemo,
  createStore,
  createRoot,
  createSignal,
  getOwner,
  runWithOwner,
  untrack,
} from "solid-js";

// ADR 0323 (review round 10): `createRoot(init)` is `runWithOwner(createOwner(),
// () => init(dispose))` (`@solidjs/signals@2.0.0-rc.13` `dist/dev-shared.js:3478-3481`)
// and `runWithOwner` swaps `context` and `tracking` and nothing else
// (`:6919-6947`). `strictRead` is set by the component body's dev
// `untrack(() => Comp(props), label)` (`:5863-5888`) and read by every reactive
// read (`:6511-6516`), so a read in either callback warns exactly where the same
// read written at the call would, and nowhere else. `untrack(fn)` sets
// `strictRead = false`: explicit intent, no warning.

// In the component body: proven violations.
export function InlineCreateRoot() {
  const [count] = createSignal(0);
  createRoot(() => console.log(count()));
  return <div />;
}
export function InlineRunWithOwner() {
  const [count] = createSignal(0);
  runWithOwner(getOwner(), () => console.log(count()));
  return <div />;
}
export function NestedRoots() {
  const [count] = createSignal(0);
  createRoot(() => {
    createRoot(() => console.log(count()));
  });
  return <div />;
}

// Props and store reads follow the same window.
export function PropsInRoot(props: { x: number }) {
  createRoot(() => console.log(props.x));
  return <div />;
}
export function StoreInRoot() {
  const [s] = createStore({ a: 1 });
  createRoot(() => console.log(s.a));
  return <div />;
}
export function PropsInRootInEvent(props: { x: number }) {
  return <div onClick={() => createRoot(() => console.log(props.x))} />;
}

// Where the call has no window: nothing is reported.
export function RootInMemo() {
  const [count] = createSignal(0);
  const m = createMemo(() => createRoot(() => count()));
  return <div>{m()}</div>;
}
export function RootInEvent() {
  const [count] = createSignal(0);
  return <div onClick={() => createRoot(() => console.log(count()))} />;
}
export function RunWithOwnerInEvent() {
  const [count] = createSignal(0);
  const owner = getOwner();
  return <div onClick={() => runWithOwner(owner, () => console.log(count()))} />;
}
export function RootInTimer() {
  const [count] = createSignal(0);
  setTimeout(() => createRoot(() => console.log(count())), 0);
  return <div />;
}
export function RunWithOwnerInThen() {
  const [count] = createSignal(0);
  const owner = getOwner();
  void Promise.resolve().then(() => runWithOwner(owner, () => console.log(count())));
  return <div />;
}
// Explicit untracked intent: `untrack` clears the window.
export function UntrackInline() {
  const [count] = createSignal(0);
  untrack(() => console.log(count()));
  return <div />;
}

// A callback a hook runs inside a root runs in the window of its caller's body:
// an obligation, never silent and never proven.
function useRoot(cb: () => void) {
  createRoot(() => cb());
}
export function HookRootSlot() {
  const [count] = createSignal(0);
  useRoot(() => console.log(count()));
  return <div />;
}
function useRunWithOwner(cb: () => void) {
  runWithOwner(getOwner(), () => cb());
}
export function HookRunWithOwnerSlot() {
  const [count] = createSignal(0);
  useRunWithOwner(() => console.log(count()));
  return <div />;
}
// A tracked memo and a synchronous root: the root invocation is in the body.
function useMemoAndRoot(cb: () => void) {
  const m = createMemo(() => (cb(), 1));
  createRoot(() => cb());
  return m;
}
export function HookMemoAndRoot() {
  const [count] = createSignal(0);
  const m = useMemoAndRoot(() => console.log(count()));
  return <div>{m()}</div>;
}
// The same hook called from an event is a different caller's window.
export function HookRootInEvent() {
  const [count] = createSignal(0);
  return <div onClick={() => useRoot(() => console.log(count()))} />;
}

// Each component above is rendered with props no tag enumerates, so they are
// unknown, not the empty props of a component nothing renders (ADR 0373).
declare const propsInRootProps: Parameters<typeof PropsInRoot>[0];
export function PropsInRootHost() {
  return <PropsInRoot {...propsInRootProps} />;
}
