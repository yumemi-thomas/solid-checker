import { Loading } from "solid-js";

export function jsxBoundary(cb: () => string) {
  return <Loading fallback={<p>wait</p>}>{cb()}</Loading>;
}
