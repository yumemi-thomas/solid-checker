import { createEffect } from "solid-js";
function install(read: () => number) {
  createEffect(() => 1, () => read());
}
export { install };
