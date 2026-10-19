import { createSignal } from "solid-js";
const [count] = createSignal(0);
export const api = {
  run() {},
  read() {
    return 1;
  },
};
export const reader = { read: () => count() };
