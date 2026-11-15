import { onCleanup } from "solid-js";

// Loaded by `lazy(() => import("./Page"))` only: the render lazy makes is its one entry.
export default function Page() {
  onCleanup(() => {});
  return <h1>page</h1>;
}
