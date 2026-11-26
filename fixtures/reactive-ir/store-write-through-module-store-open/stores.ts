import { createStore } from "solid-js";

// ADR 1232: an exported module store of an open program (the manifest is not private): code the project does not hold may import it.
export const [direct, setDirect] = createStore({ a: { v: 0 }, items: [0] });
