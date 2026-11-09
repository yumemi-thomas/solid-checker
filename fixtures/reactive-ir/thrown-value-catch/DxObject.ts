import { createSignal } from "solid-js";
const [held] = createSignal(1);
export default { c: held };
