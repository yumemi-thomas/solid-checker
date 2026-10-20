import { createSignal } from "solid-js";
import { runLater, runNow } from "./helpers";
import * as helpers from "./helpers";

function localChain(callbacks: Array<() => void>) {
  return () => {
    for (const callback of callbacks) callback();
  };
}

function eager(callbacks: Array<() => void>) {
  for (const callback of callbacks) callback();
}

function now(callback: () => void) {
  callback();
}

function ignore(_callback: () => void) {
  return 1;
}

async function afterAwait(callback: () => void) {
  await Promise.resolve();
  callback();
}

// Uncertifiable: the array element runs only from the invoker `localChain`
// returns, which runs from a click handler.
export function ChainInHandler() {
  const [n] = createSignal(0);
  const run = localChain([() => console.log(n())]);
  return <button onClick={() => run()}>{n()}</button>;
}

// Uncertifiable: the callee keeps the literal in the closure it returns.
export function KeptCrossFile() {
  const [n] = createSignal(0);
  const run = runLater(() => console.log(n()));
  return <button onClick={() => run()}>{n()}</button>;
}

// Uncertifiable: never invoked at all, and nothing proves that either.
export function Ignored() {
  const [n] = createSignal(0);
  ignore(() => console.log(n()));
  return <span>{n()}</span>;
}

// Uncertifiable: an async callee invokes it after an await.
export function AfterAwait() {
  const [n] = createSignal(0);
  void afterAwait(() => console.log(n()));
  return <span>{n()}</span>;
}

// Uncertifiable, a true defect not proven: the elements are invoked during the
// call, but an array element's invocation has no fact here.
export function EagerElements() {
  const [n] = createSignal(0);
  eager([() => console.log(n())]);
  return <span>{n()}</span>;
}

// Violation: the callee calls its parameter in its own body, during the call.
export function InvokedDuringCall() {
  const [n] = createSignal(0);
  now(() => console.log(n()));
  return <span>{n()}</span>;
}

// Violation, cross-file.
export function InvokedCrossFile() {
  const [n] = createSignal(0);
  runNow(() => console.log(n()));
  return <span>{n()}</span>;
}

// Violation: a project object's method that calls its parameter.
const bus = {
  run(callback: () => void) {
    callback();
  },
};
export function InvokedByMethod() {
  const [n] = createSignal(0);
  bus.run(() => console.log(n()));
  return <span>{n()}</span>;
}

// Violation, through a namespace import of the same helper.
export function InvokedThroughNamespace() {
  const [n] = createSignal(0);
  helpers.runNow(() => console.log(n()));
  return <span>{n()}</span>;
}
