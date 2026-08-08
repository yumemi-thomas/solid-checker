import { createEffect } from "solid-js";
function install(value: Readonly<{ name: string }>) {
  createEffect(() => value.name);
}
export { install };
