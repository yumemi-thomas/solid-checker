import { createSignal } from "solid-js";
const [count] = createSignal(0);
export const api = {
  read() {
    return count();
  },
};
