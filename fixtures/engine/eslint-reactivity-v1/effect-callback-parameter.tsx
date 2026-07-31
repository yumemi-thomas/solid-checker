import { createEffect, type Accessor } from "solid-js";
function install(read: Accessor<number>) {
  createEffect(() => read());
}
export { install };
