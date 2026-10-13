import type { ComponentProps } from "solid-js";
import { RouterContext, useLocation, useNavigate } from "@solidjs/router";

// `useLocation`'s contract states its claims only while the router provides
// its own context. `Fake` below provides one, so the import of `useLocation`
// is uncertifiable: SC9005, an error, never clean. `useNavigate` states no
// premise and keeps its claims.
export function Crumb() {
  const location = useLocation();
  const navigate = useNavigate();
  return <a onClick={() => navigate("/")}>{location.pathname}</a>;
}

// The project provides the router's context itself, with a value the router
// did not build.
export function Fake(props: ComponentProps<typeof RouterContext>) {
  return <RouterContext value={props.value}>{props.children}</RouterContext>;
}
