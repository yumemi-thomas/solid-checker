import { affects, createMemo } from "solid-js";

function pick(): string {
  return "k" + String(Date.now() % 1);
}

// A key nothing decides, written on the prototype every accessor reads through: it may be `$TARGET`'s.
(Function.prototype as any)[pick()] = () => 1;

export function Keyed() {
  const m = createMemo(() => 1);
  return <button onClick={() => { affects(m); }}>go</button>;
}
