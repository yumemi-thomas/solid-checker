// Positive: a lazy glob is a dynamic import, evaluated in a later job; it orders
// nothing, so `lazy-b.tsx` still runs after this module's body.
import { createSignal } from "solid-js";

const [, setValue] = createSignal(0);

export const helper = () => {
  setValue(1);
};
export const pages = import.meta.glob("./lazy-b.tsx");
export const pagesFalse = import.meta.glob("./lazy-b.tsx", { eager: false });
