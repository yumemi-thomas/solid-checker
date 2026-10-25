import { createSignal } from "solid-js";
import { afterTick, later, withTx } from "./tx";

// The literal handed to a helper runs where the helper runs it: the role of
// each invocation of the helper's parameter, which is the role of the call
// that handed it over.

// ---- Violations: the call runs in the component's own body.

export function InBody() {
  const [, setX] = createSignal(0);
  withTx(() => setX(1));
  return <div />;
}

// A function the body calls is an owned entry too.
export function ViaSaveBody() {
  const [, setX] = createSignal(0);
  const save = () => withTx(() => setX(2));
  save();
  return <div />;
}

// A named function handed to the helper from the body.
export function NamedInBody() {
  const [, setX] = createSignal(0);
  const reset = () => setX(3);
  withTx(reset);
  return <div />;
}

// ---- Clean: every call that hands the helper this literal runs in an event.

export function InHandler() {
  const [, setX] = createSignal(0);
  return <button onClick={() => withTx(() => setX(4))} />;
}

// A function only a handler calls: the helper is called from a body elsewhere
// (`InBody`), but this literal is handed over by this activation alone.
export function ViaSave() {
  const [, setX] = createSignal(0);
  const save = () => withTx(() => setX(5));
  return <button onClick={() => save()} />;
}

export function NamedInHandler() {
  const [, setX] = createSignal(0);
  const reset = () => setX(6);
  return <button onClick={() => withTx(reset)} />;
}

// The timer runs the literal on a fresh stack, whoever scheduled it.
export function InTimer() {
  const [, setX] = createSignal(0);
  afterTick(() => setX(7));
  return <div />;
}

// ---- Obligations: the helper keeps the callback.

export function Stored() {
  const [, setX] = createSignal(0);
  later(() => setX(8));
  return <div />;
}
export function NamedStored() {
  const [, setX] = createSignal(0);
  const reset = () => setX(9);
  later(reset);
  return <div />;
}
