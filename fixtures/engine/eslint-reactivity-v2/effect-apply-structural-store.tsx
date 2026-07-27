import { createEffect } from "solid-js";
function install(value: Readonly<{ name: string }>) {
  createEffect(() => value, current => current.name);
}
export { install };
