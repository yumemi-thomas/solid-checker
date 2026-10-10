import { createMemo, createSignal, createTrackedEffect, onCleanup, onSettled, untrack } from "solid-js";
import { connect, createDataStream } from "./hook";

function field(label: string, control: unknown) {
  return [label, control];
}

// ---- An event handler is never run while the component body runs, whatever
// expression wraps the JSX that carries it.

export function HandlerInLogicalAnd() {
  const [on] = createSignal(false);
  const [x, setX] = createSignal(0);
  return <main>{on() && <button onClick={() => setX(x() + 1)}>a</button>}</main>;
}
export function HandlerInTernary() {
  const [on] = createSignal(false);
  const [x, setX] = createSignal(0);
  return <main>{on() ? <button onClick={() => setX(1)}>a</button> : <div />}</main>;
}
export function HandlerInMap() {
  const [x, setX] = createSignal(0);
  return <main>{[1, 2].map((i) => <button onClick={() => setX(i)}>{x()}</button>)}</main>;
}
export function HandlerInIife() {
  const [x, setX] = createSignal(0);
  return <main>{(() => <button onClick={() => setX(1)}>a</button>)()}</main>;
}
export function HandlerInCallArgument() {
  const [x, setX] = createSignal(0);
  return <main>{field("label", <button onClick={() => setX(2)}>b</button>)}</main>;
}

// ---- A closure nested in a leaf scope (`createTrackedEffect`, `onSettled`) is
// either deferred or runs inside the leaf; both are legal for a write.

export function TimerInTrackedEffect() {
  const [c, setC] = createSignal(0);
  createTrackedEffect(() => {
    window.setTimeout(() => setC(3), 5);
  });
  return <div>{c()}</div>;
}
export function ListenerInTrackedEffect() {
  const [c, setC] = createSignal(0);
  createTrackedEffect(() => {
    const listener = () => setC(4);
    document.addEventListener("click", listener);
    return () => document.removeEventListener("click", listener);
  });
  return <div>{c()}</div>;
}
export function ContinuationInTrackedEffect() {
  const [c, setC] = createSignal(0);
  createTrackedEffect(() => {
    Promise.resolve(5).then((value) => setC(value));
  });
  return <div>{c()}</div>;
}
export function TimerInSettled() {
  const [c, setC] = createSignal(0);
  onSettled(() => {
    window.setTimeout(() => setC(3), 5);
  });
  return <div>{c()}</div>;
}

// ---- A write after an `await` in a hook called from a component body has no
// owner (hook.ts); the write before the first `await` still runs in the body.

export function WriteAfterAwaitInHook() {
  const stream = createDataStream();
  return <div>{stream.ok() ? "on" : "off"}</div>;
}

// ---- A cleanup callback runs at disposal, and a closure kept in a memo's
// result runs when something calls it, never while the memo computes.

export function WriteInCleanup() {
  const [c, setC] = createSignal(0);
  onCleanup(() => setC(0));
  return <div>{c()}</div>;
}
export function ClosureStoredInMemo() {
  const [tab, setTab] = createSignal("a");
  const actions = createMemo(() => ({ go: () => setTab("b") }));
  return <button onClick={() => actions().go()}>{tab()}</button>;
}

// ---- Positive controls: each runs in an owned scope, so the write is a proven
// owned-scope write.

export function WriteInBody() {
  const [c, setC] = createSignal(0);
  setC(1);
  return <div>{c()}</div>;
}
export function HelperCalledInBody() {
  const [c, setC] = createSignal(0);
  function reset() {
    setC(0);
  }
  reset();
  return <div>{c()}</div>;
}
export function WriteBeforeAwaitInHook() {
  const live = connect();
  return <div>{live() ? "on" : "off"}</div>;
}
export function WriteInMemoBody() {
  const [c, setC] = createSignal(0);
  const next = createMemo(() => {
    setC(1);
    return c() + 1;
  });
  return <div>{next()}</div>;
}
export function WriteInImmediatelyInvokedFunction() {
  const [c, setC] = createSignal(0);
  (() => setC(2))();
  return <div>{c()}</div>;
}

// ---- A closure handed to a function that is not proven to call it during the
// call (a callback prop, a parent's ref receiver) runs when that function says.

export function ClosureHandedToCallbackProp(props: { settings?: (close: () => void) => string }) {
  const [open, setOpen] = createSignal(false);
  return <div>{open() && props.settings?.(() => setOpen(false))}</div>;
}
export function NamedFunctionHandedToParent(props: { receive?: (ask: () => void) => void }) {
  const [asking, setAsking] = createSignal(true);
  function ask() {
    setAsking(true);
  }
  untrack(() => props.receive)?.(ask);
  return <div>{String(asking())}</div>;
}

// ---- Reads, not writes: an async memo read inside a callback prop's closure is
// a read in what the child calls later, not in the JSX region's tracking pass
// (so there is no render of the pending value without a Loading boundary).

declare function fetchAuthor(): Promise<string>;
function Card(props: { onDownload?: () => void }) {
  return <button onClick={props.onDownload}>x</button>;
}
export function CallbackPropReadsAsyncMemo(props: { show: boolean }) {
  const author = createMemo(async () => fetchAuthor());
  return <div>{props.show && <Card onDownload={() => console.log(author())} />}</div>;
}
