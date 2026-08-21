import { createSignal } from "solid-js";

// A module-scope source other files read through helpers. Cross-file flow is
// the shape the old two-file `interprocedural` fixture could not express.
export const [count, setCount] = createSignal(0);
