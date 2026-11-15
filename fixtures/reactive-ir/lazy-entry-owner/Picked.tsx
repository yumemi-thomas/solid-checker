import { onCleanup } from "solid-js";

// The loader reaches `lazy` through a variable: the facts name no read of this module, so its namespace escapes with no name. Open.
export default function Picked() {
  onCleanup(() => {});
  return <h3>picked</h3>;
}
