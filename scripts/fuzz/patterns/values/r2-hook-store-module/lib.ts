import { createStore } from "solid-js";
const [st] = createStore({ v: 1 });
export function use() { return st; }
