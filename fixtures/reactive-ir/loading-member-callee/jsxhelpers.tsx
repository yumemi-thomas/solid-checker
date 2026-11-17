import { Loading } from "solid-js";

// A callee that holds its calls of the parameter in a `Loading` element: Chrome is quiet at a read in the closure.
export function jsxBoundary(cb: () => string) {
  return <Loading fallback={<p>wait</p>}>{cb()}</Loading>;
}
