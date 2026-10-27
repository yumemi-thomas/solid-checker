import { Errored, Loading, Loading as LoadingAlias, runWithOwner } from "solid-js";
import * as S from "solid-js";

const { Errored: ErroredD } = S;

// Positive: an event handler runs with no owner.
export function Handler() {
  return <button onClick={() => Loading({ fallback: 0, children: 1 })}>go</button>;
}

// Positive: a timer callback runs with no owner; `Errored` is the error boundary.
export function Timer() {
  setTimeout(() => Errored({ fallback: 0, children: 1 }), 0);
  return <i>timer</i>;
}

// Positive: a `Promise.then` callback, through an aliased import.
export function Then() {
  Promise.resolve().then(() => LoadingAlias({ fallback: 0, children: 1 }));
  return <i>then</i>;
}

// Positive: a microtask, through a namespace member.
export function Micro() {
  queueMicrotask(() => S.Loading({ fallback: 0, children: 1 }));
  return <i>micro</i>;
}

// Positive: the code after an `await` is a continuation with no owner,
// through a destructuring of the namespace (ADR 0458).
export function AfterAwait() {
  const load = async () => {
    await Promise.resolve();
    ErroredD({ fallback: 0, children: 1 });
  };
  void load();
  return <i>await</i>;
}

// Positive: `runWithOwner(null, ...)` runs its callback with no owner.
export function OwnerNull() {
  runWithOwner(null, () => {
    Loading({ fallback: 0, children: 1 });
  });
  return <i>null</i>;
}

// Positive: a project wrapper whose only caller is a timer callback.
function makeBoundary() {
  return Errored({ fallback: 0, children: 1 });
}
export function Wrapper() {
  setTimeout(() => makeBoundary(), 0);
  return <i>wrapper</i>;
}

// Negative: a component body has an owner.
export function Body() {
  const view = Loading({ fallback: 0, children: 1 });
  return <i>{view}</i>;
}

// Negative: a local function named `Loading` is not Solid's.
export function Shadowed() {
  const Loading = (props: { children: number }) => props.children;
  return <button onClick={() => Loading({ children: 1 })}>shadowed</button>;
}
