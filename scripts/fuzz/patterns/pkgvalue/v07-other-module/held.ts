import { createSignal } from "solid-js";
import { view } from "fz-view";
const [count] = createSignal(1);
export const location = view(count);
