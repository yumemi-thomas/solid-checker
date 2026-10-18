import { collection } from "./factory";

function timer() {
  const source = collection(1);
  setTimeout(() => { void source.rows(); }, 0);
}
export function Timer() { timer(); return <p />; }

function microtask() {
  const source = collection(1);
  queueMicrotask(() => { void source.selected(); });
}
export function Microtask() { microtask(); return <p />; }

function wrapped() {
  const source = collection(1);
  (queueMicrotask as typeof queueMicrotask)(() => { void source.rows(); });
}
export function Wrapped() { wrapped(); return <p />; }

function helper() {
  const source = collection(1);
  function read() { return source.rows(); }
  queueMicrotask(() => { void read(); });
}
export function Helper() { helper(); return <p />; }

function nestedInline() {
  const source = collection(1);
  queueMicrotask(() => { [1].forEach(() => { void source.rows(); }); });
}
export function NestedInline() { nestedInline(); return <p />; }

declare function opaque(callback: () => void): void;
function nestedOpaque() {
  const source = collection(1);
  queueMicrotask(() => { opaque(() => { void source.rows(); }); });
}
export function NestedOpaque() { nestedOpaque(); return <p />; }

function inline() {
  const source = collection(1);
  [1].forEach(() => { void source.rows(); });
}
export function Inline() { inline(); return <p />; }

function listener() {
  const source = collection(1);
  const target = new EventTarget();
  target.addEventListener("read", () => { void source.rows(); });
  target.dispatchEvent(new Event("read"));
}
export function Listener() { listener(); return <p />; }

declare const thenable: PromiseLike<number>;
function synchronousThenable() {
  const source = collection(1);
  void thenable.then(() => { void source.rows(); });
}
export function SynchronousThenable() { synchronousThenable(); return <p />; }

declare const promise: Promise<number>;
function receiverCatch() {
  const source = collection(1);
  void promise.catch(() => { void source.rows(); });
}
export function ReceiverCatch() { receiverCatch(); return <p />; }

declare const unknownScheduler: typeof queueMicrotask;
function unknown() {
  const source = collection(1);
  unknownScheduler(() => { void source.rows(); });
}
export function Unknown() { unknown(); return <p />; }

function shadowed() {
  const source = collection(1);
  function queueMicrotask(callback: () => void) { callback(); }
  queueMicrotask(() => { void source.rows(); });
}
export function Shadowed() { shadowed(); return <p />; }

function scheduledAndReturned() {
  const source = collection(1);
  const task = () => source.rows();
  queueMicrotask(task);
  return task;
}
export function ScheduledAndReturned() {
  const task = scheduledAndReturned();
  const value = task();
  return <p>{value}</p>;
}
