import { createEffect, type Accessor } from "solid-js";
function install(props: { read: Accessor<number> }) {
  createEffect(() => 1, () => props.read());
}
export { install };
