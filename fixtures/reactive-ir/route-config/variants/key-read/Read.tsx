import { useRouteMatches } from "@solidjs/router";

// A project read of a route's `component`: the value read back could be
// called by the project, which the census does not see.
export function first() {
  const matches = useRouteMatches();
  return matches()[0]?.route.component;
}
