import { createSignal } from "solid-js";
const [count] = createSignal(1);
export const shared = () => console.log(count());
window.addEventListener("click", shared);
