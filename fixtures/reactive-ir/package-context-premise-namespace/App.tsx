import { useContext } from "solid-js";
import * as Router from "@solidjs/router";

// Through a namespace import. `useContext(Router.RouterContext)` is a read and
// allowed; handing `Router.RouterContext` to a function this analysis does
// not follow is a provision it cannot rule out, so the namespace member
// `Router.useLocation` is uncertifiable.
declare function register(context: unknown): void;

export function Crumb() {
  const location = Router.useLocation();
  const router = useContext(Router.RouterContext);
  return <span data-base={router.base.path()}>{location.pathname}</span>;
}

register(Router.RouterContext);
