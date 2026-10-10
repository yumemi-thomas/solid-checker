import { createMemo, createSignal } from "solid-js";

const [count] = createSignal(0);
const list = [1, 2];

declare const element: HTMLElement;
declare const input: HTMLInputElement;
declare const target: EventTarget;
declare const thenable: PromiseLike<number>;

// Positive (violation): a read in the component body runs inside the
// strict-read window.
function Body() {
  console.log(count());
  return <div />;
}

// Uncertifiable: a host API retains each of these callbacks and may invoke it
// on its invoker's stack -- inside the component body's strict-read window when
// the body hands control back synchronously, after it otherwise.
function ElementListener() {
  element.addEventListener("click", () => console.log(count()));
  return <div />;
}

function InputListener() {
  input.addEventListener("input", () => console.log(count()));
  return <div />;
}

function TargetListener() {
  target.addEventListener("change", () => console.log(count()));
  return <div />;
}

function WindowListener() {
  window.addEventListener("resize", () => console.log(count()));
  return <div />;
}

function DocumentListener() {
  document.addEventListener("keydown", () => console.log(count()));
  return <div />;
}

function Thenable() {
  thenable.then(() => console.log(count()));
  return <div />;
}

function Bound() {
  const log = (read: () => number) => console.log(read());
  log.bind(null, () => count());
  return <div />;
}

function Geolocation() {
  navigator.geolocation.getCurrentPosition(() => console.log(count()));
  return <div />;
}

// Uncertifiable: a standard-library inline callback inside the listener runs
// where the listener does, and a read through a local helper called there too.
function ListenerForEach() {
  element.addEventListener("click", () => list.forEach(() => console.log(count())));
  return <div />;
}

function ListenerHelper() {
  const read = () => count();
  element.addEventListener("click", () => console.log(read()));
  return <div />;
}

// Uncertifiable, not proven: the body dispatches synchronously after
// registering, which does run the listener inside the window, but the checker
// does not prove a dispatch reaches a registered listener (same target, same
// event type, no removal in between, on every path).
function DispatchedInBody() {
  target.addEventListener("ping", () => console.log(count()));
  target.dispatchEvent(new Event("ping"));
  return <div />;
}

// Uncertifiable SC5001: inside the window a pending read throws
// PENDING_ASYNC_UNTRACKED_READ, after it a plain NotReadyError.
function ListenerAsync() {
  const user = createMemo(async () => 1);
  element.addEventListener("click", () => console.log(user()));
  return <div />;
}

// Negatives: a fresh-stack scheduler runs its callback after the body returned,
// and a JSX event handler is the compiler's event callback.
function Timer() {
  setTimeout(() => console.log(count()), 0);
  return <div />;
}

function Handler() {
  return <button onClick={() => console.log(count())} />;
}

// Positive (violation): a project method spelled `addEventListener` is not the
// default-library declaration, and this one calls its listener in the body.
const bus = {
  addEventListener(_type: string, listener: () => void) {
    listener();
  },
};
function LocalBus() {
  bus.addEventListener("tick", () => console.log(count()));
  return <div />;
}
