import { onCleanup } from "solid-js";

// Loaded by `lazy(() => import("./Page"))` only, as its `default`: the render `lazy` makes is its one entry.
export default function Page(props: { title: string }) {
  onCleanup(() => {});
  return <h1>{props.title}</h1>;
}
