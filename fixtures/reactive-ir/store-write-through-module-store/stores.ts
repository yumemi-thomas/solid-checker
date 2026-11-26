import { createStore } from "solid-js";

// ADR 1232: module-scope stores another module exports.
export const [direct, setDirect] = createStore({ a: { v: 0 }, items: [0] });
export const [viaAlias, setViaAlias] = createStore({ a: { v: 0 }, items: [0] });
export const [kept, setKept] = createStore({ a: { v: 0 }, items: [0] });
export const [called, setCalled] = createStore({ a: { v: 0 }, items: [0] });
