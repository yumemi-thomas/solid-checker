import { createSignal } from "solid-js";

// ---- Runs: the carrier calls the callback; the write is a violation (Chrome: REACTIVE_WRITE_IN_OWNED_SCOPE) ----

export function NonEmpty() {
  const [, setW] = createSignal(0);
  const tf = () => {
    setW(10);
  };
  [1].forEach(tf);
  return <p />;
}
export function ReduceWithInitial() {
  const [, setW] = createSignal(0);
  const tf = (a: number) => {
    setW(12);
    return a;
  };
  [1].reduce(tf, 0);
  return <p />;
}
export function SetWithElement() {
  const [, setW] = createSignal(0);
  const tf = () => {
    setW(13);
  };
  new Set([1]).forEach(tf);
  return <p />;
}
export function ReplaceMatch() {
  const [, setW] = createSignal(0);
  const tf = () => {
    setW(14);
    return "y";
  };
  "x".replace("x", tf);
  return <p />;
}
// The empty carrier is one reference; another reference calls the function.
export function AlsoCalled() {
  const [, setW] = createSignal(0);
  const tf = () => {
    setW(15);
  };
  ([] as number[]).forEach(tf);
  tf();
  return <p />;
}
