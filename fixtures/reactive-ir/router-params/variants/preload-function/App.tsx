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

// Withheld: the router renders `<Root routerState={routerState} ...
// preload={config.preload}>` and calls `props.preload({ ... })` on Root's
// props (`dist/routers/factory.jsx:295`, `components.jsx:13`), so a
// `function` there runs with `this.routerState`, the router context, and
// replaces its `wrapParams` through a key no spelling names.
const key = ["wrap", "Params"].join("");
export const Router = createRouter({
  routes: [],
  preload: function (this: { routerState: Record<string, unknown> }) {
    this.routerState[key] = (getParams: () => unknown) => getParams();
  },
});
