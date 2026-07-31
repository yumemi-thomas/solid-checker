import { createEffect, type Accessor } from "solid-js";
function install(props: { read: Accessor<number> }) {
  createEffect(() => props.read());
}
export { install };
