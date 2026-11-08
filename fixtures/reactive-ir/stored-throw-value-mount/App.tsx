import { Errored } from "solid-js";
import { Dynamic } from "@solidjs/web";
import { Handled, Reported, Routed, Unused } from "./kid";

// Obligations: mounted only as a value in this file, under a boundary that takes the throw, or nowhere this reads.
export function HandlingBoundary() {
  return <Errored fallback={(e) => <i>{String(e())}</i>}><Dynamic component={Handled} /></Errored>;
}

const routes = [{ view: Routed }];

export function RouteTable() {
  const View = routes[0].view;
  return <Errored fallback={(e) => <i>{String(e())}</i>}><View /></Errored>;
}

// Violation: `Dynamic` with no boundary: Chrome reports the throw.
export function NoBoundary() {
  return <Dynamic component={Reported} />;
}

export { Unused };
