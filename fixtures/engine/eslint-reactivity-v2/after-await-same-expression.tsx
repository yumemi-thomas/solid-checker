import { createMemo, createSignal } from "solid-js";
const [count] = createSignal(0);
export const value = createMemo(async () => (await Promise.resolve(), count()));
