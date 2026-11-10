import { createSignal } from "solid-js";

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

// The guard is the global `Array.isArray`: the list is a proven array, so a task pushed into it is retained, and the call
// is an obligation (SC9012) beside the read's own (SC1001 uncertifiable). The task really runs on the spot (`Runner`), so
// the read is no clean.
export function Real() {
  const [count] = createSignal(0);
  register(new Runner(), () => count());
  return <div />;
}
