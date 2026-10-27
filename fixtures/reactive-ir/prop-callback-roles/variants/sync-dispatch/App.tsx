import { createSignal } from "solid-js";
import { Direct, InHandler } from "./children";

// A component body dispatches an event synchronously, on an owned stack: any
// handler may run there, under the body's owner (ADR 0356), so what a handler
// does is not proven ownerless anywhere in the program.
export function Dispatches() {
  const target = document.createElement("input");
  target.dispatchEvent(new Event("change"));
  return <main />;
}

export function Writes() {
  const [, setN] = createSignal(0);
  return <button onClick={() => setN(1)} />;
}

export function UsesDirect() {
  const [, setN] = createSignal(0);
  return <Direct onPress={() => setN(1)} />;
}

export function UsesInHandler() {
  const [, setN] = createSignal(0);
  return <InHandler onPress={() => setN(1)} />;
}
