import { cycled } from "./cycle-a";

// Negative: cycle-a.ts and cycle-b.tsx import each other. Evaluating cycle-b
// first, cycle-a runs before cycle-b's body, but evaluating cycle-a first runs
// cycle-b's body before `cycled` is declared. The call may reach a
// temporal dead zone, so no module order proves it runs.
export function CycleCaller() {
  cycled();
  return <button>cycle</button>;
}
