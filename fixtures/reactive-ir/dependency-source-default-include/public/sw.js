// A project JavaScript file. The project does not set `allowJs`, so it is not
// in the program, and the mode must not pull it in.
import { createSignal, createEffect } from "solid-js";
const [n] = createSignal(0);
createEffect(() => n(), () => {});
export const x = n();
