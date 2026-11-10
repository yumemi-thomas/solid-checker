import { createSignal } from "solid-js";

// This module's own `Array` shadows the global one: its `isArray` proves nothing about the list.
const Array: any = { isArray: (_value: unknown) => true };

class Runner {
  push(task: () => void) {
    task();
  }
}

function register(list: any, task: () => void) {
  if (Array.isArray(list)) {
    list.push(task);
  }
}

// No proven array, so no retained-value obligation: only the read's own (SC1001 uncertifiable). The task runs on the spot.
export function Shadow() {
  const [count] = createSignal(0);
  register(new Runner(), () => count());
  return <div />;
}
