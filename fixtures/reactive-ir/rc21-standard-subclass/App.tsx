import { createSignal } from "solid-js";

const [c] = createSignal(1);

// Runs its callback on a microtask, whatever the type below says.
class Late extends Array<number> {
  override forEach(cb: (v: number, i: number, a: number[]) => void) {
    queueMicrotask(() => { super.forEach(cb); });
  }
}

export function App() {
  const held: number[] = new Late();
  // May be the override: not proven in place, so not proven in a window.
  held.forEach(() => { c(); });
  // A literal is no instance of a subclass: the warning stands.
  [1].forEach(() => { c(); });
  return <p>x</p>;
}
