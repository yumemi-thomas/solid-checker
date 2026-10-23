import { createSignal, createStore } from "solid-js";

const [count] = createSignal(0);
const [store] = createStore({ ready: true });
const other = { ready: false };

// CERTIFIED: the default calls `Function.prototype.toString` on the accessor,
// never `count`; its leading token made it an accessor read.
function label(text = count.toString()) {
  return text;
}
export function DefaultMemberCall() {
  const t = label();
  return <div>{t}</div>;
}

// VIOLATION: the default calls `count` while `DefaultCall` renders.
function labelRead(text = String(count())) {
  return text;
}
export function DefaultCall() {
  const t = labelRead();
  return <div>{t}</div>;
}

function run(fn: () => number) {
  return fn();
}

// UNCERTIFIABLE: `run` calls `count` here; the argument's leading token is the
// store, which was taken as the argument (a proxy is never a getter: certified).
export function StoreLedArgument() {
  const v = run(store === other ? count : count);
  return <div>{v}</div>;
}

// VIOLATION: `run(count)` reads the accessor in the body.
export function AccessorArgument() {
  const v = run(count);
  return <div>{v}</div>;
}
