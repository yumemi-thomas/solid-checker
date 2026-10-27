import { createSignal } from "solid-js";

const [, setCount] = createSignal(0);

// Written here, reached only from the click handler in App.tsx: placed when
// every importer is in the program, unplaced when an unseen package may call it.
export function bump() {
  setCount(1);
}
