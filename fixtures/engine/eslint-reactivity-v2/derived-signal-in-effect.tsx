import { createEffect, createSignal } from "solid-js";

const [, setDerived] = createSignal(0);
createEffect(() => setDerived(1), () => {});
