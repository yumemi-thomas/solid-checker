import { createEffect, createSignal } from "solid-js";
import { dynamicComponent } from "@solidjs/web";

const Plain = () => <div />;

// Clean: called only from an apply `defer: true` skips and no compute source
// can schedule (ADR 0590).
function wrapDead() { dynamicComponent(() => Promise.resolve(Plain), { static: true }); }
// Violation: a compute reading a retained writable source schedules the apply.
function wrapLive() { dynamicComponent(() => Promise.resolve(Plain), { static: true }); }

export function App() {
  const [count, setCount] = createSignal(0);
  createEffect(() => 1, () => { wrapDead(); }, { defer: true });
  createEffect(() => count(), () => { wrapLive(); }, { defer: true });
  void setCount;
  return <div />;
}
