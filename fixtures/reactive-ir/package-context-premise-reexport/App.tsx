import { useLocation } from "@solidjs/router";

// Nothing in this file provides the router's context. `routing.ts` re-exports
// it, and whoever imports that module may provide it where this analysis does
// not look, so the import of `useLocation` is uncertifiable.
export function Crumb() {
  const location = useLocation();
  return <span>{location.pathname}</span>;
}
