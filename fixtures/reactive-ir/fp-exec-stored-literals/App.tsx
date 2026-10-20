import { createSignal } from "solid-js";

function register(o: { run: () => void }) {
  return o;
}
function keep(f: () => void) {
  return f;
}
function now(f: () => number) {
  return f();
}

// ---- A function literal that is stored -- in an object, an array, a getter --
// is not invoked where it is written. Wherever it is called later (a click
// handler, a reader of the object, never) is not proven by its position, so a
// read inside is an unproven invocation (uncertifiable), not a proven
// untracked read in the body.

export function ObjectArrowProperty() {
  const [n] = createSignal(0);
  const o = { run: () => console.log(n()) };
  return <button onClick={() => o.run()}>x</button>;
}
export function ObjectMethod() {
  const [n] = createSignal(0);
  const o = { run() { console.log(n()); } };
  return <div title={typeof o.run} />;
}
export function ObjectFunctionProperty() {
  const [n] = createSignal(0);
  const o = { run: function () { console.log(n()); } };
  return <div title={typeof o.run} />;
}
export function ObjectGetter() {
  const [n] = createSignal(0);
  const o = { get value() { return n(); } };
  return <div title={typeof o} />;
}
export function ArrayElement() {
  const [n] = createSignal(0);
  const list = [() => n()];
  return <div title={String(list.length)} />;
}
export function ObjectHandedToProjectHelper() {
  const [n] = createSignal(0);
  register({ run: () => console.log(n()) });
  return <div />;
}
export function CallbackKeptByProjectHelper() {
  const [n] = createSignal(0);
  keep(() => console.log(n()));
  return <div />;
}

// ---- A default-parameter initializer runs when the function is called without
// that argument, not when the function is declared.

export function DefaultParameterDeclaration() {
  const [n] = createSignal(0);
  function capture(value = n()) {
    console.log(value);
  }
  return <button onClick={() => capture()}>x</button>;
}
export function DefaultParameterArrow() {
  const [n] = createSignal(0);
  const capture = (value = n()) => {
    console.log(value);
  };
  return <button onClick={() => capture()}>x</button>;
}

// ---- Positive controls: each of these runs while the component body runs, so
// the read is a proven untracked read.

export function ReadInBody() {
  const [n] = createSignal(0);
  const snapshot = n();
  return <div title={String(snapshot)} />;
}
export function ImmediatelyInvoked() {
  const [n] = createSignal(0);
  const snapshot = (() => n())();
  return <div title={String(snapshot)} />;
}
export function InlineStandardLibraryCallback() {
  const [n] = createSignal(0);
  const doubled = [1, 2].map((x) => x * n());
  return <div title={String(doubled.length)} />;
}
export function InvokedDuringCall() {
  const [n] = createSignal(0);
  const snapshot = now(() => n());
  return <div title={String(snapshot)} />;
}

// ---- A function bound to a name runs during the body only if every reference
// to it is a call and every call runs during the body.

export function HelperCalledInBody() {
  const [n] = createSignal(0);
  const log = () => console.log(n());
  log();
  return <div />;
}
export function HelperCalledOnlyFromHandler() {
  const [n] = createSignal(0);
  const log = () => console.log(n());
  return <button onClick={() => log()}>x</button>;
}
export function HelperCalledInBodyAndFromHandler() {
  const [n] = createSignal(0);
  const log = () => console.log(n());
  log();
  return <button onClick={() => log()}>x</button>;
}
export function HelperPassedAsValue() {
  const [n] = createSignal(0);
  function log() {
    console.log(n());
  }
  return <button onClick={log}>x</button>;
}
export function HelperNeverCalled() {
  const [n] = createSignal(0);
  const log = () => console.log(n());
  return <div title={typeof log} />;
}
export function HelperChainCalledInBody() {
  const [n] = createSignal(0);
  const inner = () => n();
  const outer = () => inner() + 1;
  const snapshot = outer();
  return <div title={String(snapshot)} />;
}

// ---- A default parameter runs when a call leaves the argument out. Called in
// the body without it, it is a read in the body.

export function DefaultParameterCalledInBody() {
  const [n] = createSignal(0);
  function capture(value = n()) {
    console.log(value);
  }
  capture();
  return <div />;
}
export function DefaultParameterCalledWithArgument() {
  const [n] = createSignal(0);
  function capture(value = n()) {
    console.log(value);
  }
  capture(1);
  return <div />;
}

// ---- A helper called only from a tracked JSX attribute runs in the compiled
// computation that attribute becomes, not in the body's strict-read window.

export function HelperCalledFromTrackedAttribute() {
  const [n] = createSignal(0);
  const label = () => String(n());
  return <div title={label()} />;
}
export function FilterInHelperCalledFromTrackedAttribute() {
  const [n] = createSignal(0);
  const visible = () => [1, 2, 3].filter((x) => x > n());
  return <div title={String(visible().length)} />;
}
