import { createStore } from "solid-js";

export const [shared, setShared] = createStore({ v: 0 });
export const [other] = createStore({ v: 0 });
