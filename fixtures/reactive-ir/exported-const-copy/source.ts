import { createSignal, createStore } from "solid-js";

const [count] = createSignal(1);
const [held] = createStore({ value: 1 });
export const copied = count;
export const copiedStore = held;
export const plain = 1;
