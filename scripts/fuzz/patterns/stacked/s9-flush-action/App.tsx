import { action, flush } from "solid-js";

export function App() {
  const act = action(function* () {
    flush();
    yield 1;
  });
  queueMicrotask(() => document.querySelector("button")?.click());
  return <button onClick={() => act()}>x</button>;
}
