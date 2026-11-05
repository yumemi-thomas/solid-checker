import { createStore } from "solid-js";
export function useThing() { const [state, setState] = createStore({ n: 1 }); return { state, setState }; }
