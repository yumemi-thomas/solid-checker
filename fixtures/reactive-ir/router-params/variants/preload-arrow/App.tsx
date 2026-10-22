import { Show } from "solid-js";
import { createRouter, useParams } from "@solidjs/router";

declare function lookup(value: unknown): number;
function fetchThing(id: string) {
  return lookup(id);
}
function Screen(props: { clusterId: string }) {
  const value = fetchThing(props.clusterId);
  return <div>{value}</div>;
}
export function RouteAccessor() {
  const params = useParams<{ clusterId?: string }>();
  return (
    <Show when={params.clusterId}>{(id) => <Screen clusterId={id()} />}</Show>
  );
}

// Kept: the router calls `props.preload({ ... })` on Root's props
// (`dist/routers/components.jsx:13`), but an arrow, inline or bound to a
// `const`, has no `this` of its own. `SC9012` is settled on
// `props.clusterId`.
const loadRoot = () => {
  lookup("root");
};
export const Router = createRouter({
  routes: [],
  preload: () => lookup("root"),
});
export const Other = createRouter({
  routes: [],
  preload: loadRoot,
});
