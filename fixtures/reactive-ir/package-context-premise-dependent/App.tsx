import { useContext } from "solid-js";
import { RouterContext, useLocation } from "@solidjs/router";

// This project's own code meets the premise, exactly as in
// `package-context-premise-met`. But `router-extension`, installed beside the
// router, declares a dependency on it, and its code may provide the context
// where this analysis does not look. So the import of `useLocation` is
// uncertifiable.
export function Crumb() {
  const location = useLocation();
  const router = useContext(RouterContext);
  return <span data-base={router.base.path()}>{location.pathname}</span>;
}
