import { createComponent, type ComponentProps } from "solid-js";
import { RouterContext, useLocation } from "@solidjs/router";

// The compiled spelling of a provider: `createComponent(RouterContext, ...)`.
// It passes the context to a call other than `useContext`, which is a
// provision, so the import of `useLocation` is uncertifiable.
export function Crumb() {
  const location = useLocation();
  return <span>{location.pathname}</span>;
}

export function Fake(props: ComponentProps<typeof RouterContext>) {
  return createComponent(RouterContext, props);
}
