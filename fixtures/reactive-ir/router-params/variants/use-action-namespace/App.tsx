import { Show } from "solid-js";
import { useParams } from "@solidjs/router";
import * as R from "@solidjs/router";

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

// Withheld: `R.useAction` is the router's `useAction` through a namespace,
// and its `function` argument runs with the router context as `this`.
const key = ["wrap", "Params"].join("");
export function Save() {
  const run = R.useAction(function (this: { r: Record<string, unknown> }) {
    this.r[key] = (getParams: () => unknown) => getParams();
    return Promise.resolve();
  } as never);
  return <button onClick={() => run()}>save</button>;
}
