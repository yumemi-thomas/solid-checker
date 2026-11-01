import { createSignal } from "solid-js";

const handle = () => {};

// Violations: the expression's IIFE runs at the element's setup, in the body.
export function ReadInSetupIife() {
  const [count] = createSignal(1);
  return <button onClick={((() => { count(); return 1; })(), handle)}>go</button>;
}

export function WriteInSetupIife() {
  const [, setCount] = createSignal(1);
  return <button onClick={((() => { setCount(7); return 1; })(), handle)}>go</button>;
}

// Legal: the sequence's last operand is the handler the element binds.
export function ReadInSequenceHandler() {
  const [count] = createSignal(1);
  return <button onClick={(0, () => { count(); })}>go</button>;
}

export function WriteInSequenceHandler() {
  const [, setCount] = createSignal(1);
  return <button onClick={(0, () => { setCount(7); })}>go</button>;
}

// Legal: a handler written directly, and one picked by a conditional.
export function ReadInHandler() {
  const [count] = createSignal(1);
  return <button onClick={() => { count(); }}>go</button>;
}

export function WriteInPickedHandler(props: { on: boolean }) {
  const [, setCount] = createSignal(1);
  return <button onClick={props.on ? () => { setCount(7); } : handle}>go</button>;
}
