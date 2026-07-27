import { createEffect, type Accessor } from "solid-js";
function install(read: Accessor<number>) {
  createEffect(() => 1, () => read());
}
export { install };
