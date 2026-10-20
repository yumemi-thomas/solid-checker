import { Dynamic } from "@solidjs/web";
import { Router } from "./Routes";

// A project component reading a `component` prop, and the router instance
// rendered as a tag in another file. Neither can reach a route definition:
// the router hands definitions out only through `useRouteMatches`,
// `RouterContext` and the instance's `routes`/`config`, none used here.
export function Frame(props: { component: () => unknown }) {
  return <Dynamic component={props.component} />;
}

export const app = () => <Router />;
