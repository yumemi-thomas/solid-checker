import { onCleanup } from "solid-js";

// Closed (ADR 1025): every reference to `Home` is a route definition's `component` the audited router renders with
// `createComponent` in the route's outlet, under an owner. The cleanup has its owner.
export function Home() {
  onCleanup(() => {});
  return <p>home</p>;
}
