import { createEffect, createSignal } from "solid-js";

const [, setValue] = createSignal(0);
createEffect(() => 1, () => setValue(1));
