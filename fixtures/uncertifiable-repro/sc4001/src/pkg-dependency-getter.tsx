// compass-ui apps/ui/src/App.tsx:56: an accessor over useLocation()'s pathname is handed to a project function.
import { useLocation } from "@solidjs/router";

function bindRouter(currentPath: () => string): void {
  console.log(currentPath());
}

export function Shell() {
  const location = useLocation();
  bindRouter(() => location.pathname);
  return <main />;
}
