import { createEffect, createMemo, createRoot, createSignal, getOwner, runWithOwner, untrack } from "solid-js";

const [signal] = createSignal(0);

// Positive: upstream badUnnamedDerivedSignal case 32. The read runs once and
// is never tracked.
createRoot(() => {
  const owner = getOwner();
  createEffect(
    () => runWithOwner(owner, () => console.log(signal())),
    () => {},
  );
});

// Positive: case 33, inside a component.
export function Component() {
  const owner = getOwner();
  const [local] = createSignal(0);
  createEffect(
    () => runWithOwner(owner, () => console.log(local())),
    () => {},
  );
  return null;
}

// Positive: a memo compute.
export function Derived() {
  const owner = getOwner();
  const value = createMemo(() => runWithOwner(owner, () => signal()));
  return <span>{value()}</span>;
}

// Positive: a JSX expression is a tracked region too.
export function Inline() {
  const owner = getOwner();
  return <span>{runWithOwner(owner, () => signal())}</span>;
}

// Negative: the callback only creates a primitive; the memo's own compute tracks.
export function Creates() {
  const owner = getOwner();
  const value = createMemo(() => runWithOwner(owner, () => createMemo(() => signal())));
  return <span>{value()()}</span>;
}

// Negative: an explicit one-time snapshot.
export function Snapshot() {
  const owner = getOwner();
  const value = createMemo(() => runWithOwner(owner, () => untrack(() => signal())));
  return <span>{value()}</span>;
}

// Negative: a clearing call written directly in an `untrack` callback is the
// explicit one-time snapshot, whatever the computations around it do.
export function InUntrack() {
  const owner = getOwner();
  const value = createMemo(() => untrack(() => runWithOwner(owner, () => signal())));
  return <span>{value()}</span>;
}

// Negative: JSX the callback writes tracks its own expression.
export function Renders() {
  const owner = getOwner();
  const view = createMemo(() => runWithOwner(owner, () => <b>{signal()}</b>));
  return <div>{view()}</div>;
}

// Negative: after an await the continuation is outside every tracking scope,
// which `reactive-read-after-await` reports and this rule does not.
export function After() {
  const owner = getOwner();
  const value = createMemo(async () => {
    await Promise.resolve();
    return runWithOwner(owner, () => signal());
  });
  return <span>{String(value())}</span>;
}
