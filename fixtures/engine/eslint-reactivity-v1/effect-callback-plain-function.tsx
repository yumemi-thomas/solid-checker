import { createEffect } from "solid-js";
function install(read: () => number) {
  createEffect(() => read());
}
export { install };
