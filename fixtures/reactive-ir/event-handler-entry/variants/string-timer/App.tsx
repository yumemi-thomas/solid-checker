import { resolve } from "solid-js";

// ADR 1089 part E round 2: a timer handed a value no fact proves a function may run a string as code, which may read the handler
// Solid stored on the element: the realm is not plain for the question, the row stays.
const code: unknown = (globalThis as unknown as { code?: unknown }).code;
setTimeout(code as TimerHandler, 0);

export function Resolve() {
  return <button onClick={() => { resolve(() => 1); }}>go</button>;
}
