import { createEffect, createSignal, untrack } from "solid-js";

const [count] = createSignal(0);
const hit = () => {};

// Positive control: a bare identifier names its function; the DOM calls it as the handler with no owner.
function onHit() { createEffect(() => count(), () => {}); }
export function IdentifierHandler() {
  return <button onClick={onHit} />;
}

// Positive control: a function literal written as the handler is the handler.
export function LiteralHandler() {
  return <button onClick={() => { createEffect(() => count(), () => {}); }} />;
}

// Positive control: element 0 of a handler tuple is the handler the DOM calls (ADR 0326/0327, `review-r7-event-array`).
export function TupleHandler() {
  return <button onClick={[(data: number) => { void data; createEffect(() => count(), () => {}); }, 1]} />;
}

// Positive control: a named apply callback runs from the flush with no owner.
function applyNamed(value: number) { void value; createEffect(() => count(), () => {}); }
export function NamedApply() {
  createEffect(() => count(), applyNamed);
  return <i />;
}

// Negative: an IIFE is the handler's value, evaluated while the component renders.
export function IifeHandler() {
  return <button onClick={(() => { createEffect(() => count(), () => {}); return hit; })()} />;
}

// Negative: untrack runs its callback during the call, under the component's owner; the handler is its result.
export function UntrackHandler() {
  return <button onClick={untrack(() => { createEffect(() => count(), () => {}); return hit; })} />;
}

// Negative: a factory a handler calls runs its body while the component renders.
function makeHandler(read: () => number): () => void {
  createEffect(read, () => {});
  return hit;
}
export function FactoryHandler() {
  return <button onClick={makeHandler(count)} />;
}

// Negative: an IIFE is an effect's apply callback value, evaluated while the effect is created.
export function IifeApply() {
  createEffect(() => count(), (() => { createEffect(() => count(), () => {}); return (value: number) => { void value; }; })());
  return <i />;
}

// Negative: a factory whose result is the apply callback runs its body while the effect is created.
function makeApply(read: () => number): (value: number) => void {
  createEffect(read, () => {});
  return (value: number) => { void value; };
}
export function FactoryApply() {
  createEffect(() => count(), makeApply(count));
  return <i />;
}
