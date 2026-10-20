import { createSignal } from "solid-js";

function logThis(this: { onClick?: () => void }) {
  this.onClick?.();
}

function pick(cb: () => void, flag: boolean) {
  if (flag) return { onClick: cb, log() {} };
  return { onClick: cb, log: logThis };
}

export function App() {
  const [count] = createSignal(0);
  const r = pick(() => console.log(count()), Math.random() > 0.5);
  r.log();
  return <button {...r}>x</button>;
}

function pickOne(cb: () => void) {
  return { onClick: cb, log: logThis };
}

export function Control() {
  const [count] = createSignal(0);
  const r = pickOne(() => console.log(count()));
  r.log();
  return <button {...r}>y</button>;
}
