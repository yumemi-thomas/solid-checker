import { createEffect, createSignal, untrack } from "solid-js";

const [count] = createSignal(0);
const hit = () => {};

// H1: the handler is the value `makeHandler(count)` returns, not the factory.
// The factory runs while the component renders, so its effect is owned.
function makeHandler(read: () => number): () => void {
  createEffect(read, () => {});
  return () => {};
}
export function H1() {
  return <button onClick={makeHandler(count)} />;
}

// H2: the apply callback is the function `makeApply(count)` returns. The
// factory's own effect runs while the effect is created, so it is owned.
function makeApply(read: () => number): (value: number) => void {
  createEffect(read, () => {});
  return (value: number) => {
    void value;
  };
}
export function H2() {
  createEffect(count, makeApply(count));
  return <i />;
}

// I1: an IIFE handler runs while rendering and returns `hit`, which is not
// the literal the IIFE contains.
export function I1() {
  return (
    <button
      onClick={(() => {
        createEffect(count, () => {});
        return hit;
      })()}
    />
  );
}

// I2: `untrack` returns its callback's result, not the callback.
export function I2() {
  return (
    <button
      onClick={untrack(() => {
        createEffect(count, () => {});
        return hit;
      })}
    />
  );
}
