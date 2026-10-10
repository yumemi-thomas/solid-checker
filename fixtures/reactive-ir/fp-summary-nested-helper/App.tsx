import { createSignal, createMemo } from "solid-js";

// ---- Negatives: a helper nested in a component whose call is not written in
// the body. The helper's own read runs when the helper does, so a call to it
// from a tracked position or a handler is not a strict-window read.

// Derived chain read only through a JSX child expression, which tracks.
export function ChainInJsx() {
  const [s] = createSignal(0);
  const a = () => s() + 1;
  const b = () => a() * 2;
  return <div>{b()}</div>;
}

// A memo in the middle of the chain; the helper is still only read in JSX.
export function ChainThroughMemo() {
  const [s] = createSignal(0);
  const a = createMemo(() => s() + 1);
  const b = () => a() * 2;
  return <div>{b()}</div>;
}

// The helper is handed to a handler by reference.
export function HandlerByReference() {
  const [own, setOwn] = createSignal(0);
  const values = () => own() + 1;
  const toggle = () => {
    setOwn(values() + 1);
  };
  return <button onClick={toggle}>x</button>;
}

// The helper is called from a handler literal.
export function HandlerLiteral() {
  const [own] = createSignal(0);
  const values = () => own() + 1;
  const toggle = () => console.log(values());
  return <button onClick={() => toggle()}>x</button>;
}

// Stored in an object that is only passed around: nothing calls it here.
function use(value: unknown) {
  return value;
}
export function StoredNotCalled() {
  const [own] = createSignal(0);
  const values = () => own() + 1;
  const toggle = () => console.log(values());
  use({ toggle });
  return <div />;
}

// A function declaration helper, never called in the body.
export function DeclarationHelper() {
  const [s] = createSignal(0);
  function read() {
    return s() * 2;
  }
  return <button onClick={read}>x</button>;
}

// A helper called from a closure that is only stored: nothing proves the
// closure runs while the body does, so the body is not charged with the read.
export function StoredInProperty() {
  const [s] = createSignal(0);
  const a = () => s() + 1;
  const api = { go: () => a() };
  return <button onClick={api.go}>x</button>;
}

export function StoredInArray() {
  const [s] = createSignal(0);
  const a = () => s() + 1;
  const list = [() => a()];
  return <button onClick={list[0]}>x</button>;
}

// ---- Positive controls: the same helpers, proven called in the body. These
// stay proven violations.

// A one-level helper called while the body runs.
export function CalledInBody() {
  const [s] = createSignal(0);
  const read = () => s() + 1;
  const snapshot = read();
  return <span>{snapshot}</span>;
}

// A chain called while the body runs: the read is reached through both.
export function ChainCalledInBody() {
  const [s] = createSignal(0);
  const a = () => s() + 1;
  const b = () => a() * 2;
  const snapshot = b();
  return <span>{snapshot}</span>;
}

// A declaration helper called while the body runs.
export function DeclarationCalledInBody() {
  const [s] = createSignal(0);
  function read() {
    return s() * 2;
  }
  const snapshot = read();
  return <span>{snapshot}</span>;
}

// An immediately invoked literal runs where it is written, in the body.
export function ImmediatelyInvoked() {
  const [s] = createSignal(0);
  const a = () => s() + 1;
  const snapshot = (() => a())();
  return <span>{snapshot}</span>;
}

// The queue-management-ui shape: a prop read once while the body runs.
export function Button(props: { variant?: string }) {
  const variant = props.variant ?? "primary";
  return <button disabled={variant === "x"}>go</button>;
}

// A caller passes a reactive expression, which is what makes the one-time read
// above a proven defect rather than a possible one.
export function Toolbar() {
  const [filter] = createSignal("all");
  return <Button variant={filter() === "all" ? "primary" : "outline"} />;
}
