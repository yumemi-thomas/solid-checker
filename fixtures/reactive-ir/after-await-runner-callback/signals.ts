import { createSignal, createStore } from "solid-js";

export const [count, setCount] = createSignal(0);
export const [state, setState] = createStore({ n: 0 });
export const pause = () => Promise.resolve();
export const reads = () => count();
