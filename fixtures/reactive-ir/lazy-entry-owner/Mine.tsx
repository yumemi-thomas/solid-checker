import { onCleanup } from "solid-js";

// Loaded by a local function named `lazy`, which is no Solid `lazy` (the import binding decides, not the name): it reads nothing from the module. Open.
export default function Mine() {
  onCleanup(() => {});
  return <p>mine</p>;
}
