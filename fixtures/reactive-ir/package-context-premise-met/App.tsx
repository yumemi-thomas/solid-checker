import { createContext, useContext } from "solid-js";
import { RouterContext, useLocation, useNavigate } from "@solidjs/router";

// The premise holds. This project reads the router's context only as the
// argument of `useContext` and never provides a value for it, so the claims
// `useLocation`'s contract states under that premise apply: clean.
export function Crumb() {
  const location = useLocation();
  const navigate = useNavigate();
  const router = useContext(RouterContext);
  return <a onClick={() => navigate(router.base.path())}>{location.pathname}</a>;
}

// A local context spelled `RouterContext` is another symbol. Providing it
// provides nothing of the router's, and must not cost `useLocation` its claims.
export function Local() {
  const RouterContext = createContext<string>("local");
  return <RouterContext value="x">{useContext(RouterContext)}</RouterContext>;
}
