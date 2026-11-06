import { createMemo, createSignal, Loading, For, Show } from "solid-js";
import { Dynamic } from "@solidjs/web";

export function App() {
  const a1 = createMemo(async () => 1);
  const wrapUse = () => { a1(); };
  return <button onClick={() => { wrapUse(); }}>x</button>;
}
