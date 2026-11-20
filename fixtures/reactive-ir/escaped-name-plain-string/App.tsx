import { createSignal } from "solid-js";

export function BodyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  const text = "a\nb \u00e9 \t";
  const table = { "\n": 1, "\u00e9": 2, plain: 3 };
  const tagged = String.raw`c\u006cick`;
  const joined = `a${text}\u0063`;
  console.log(text, table, tagged, joined, "\u00a0");
  return <div>{s()}</div>;
}
