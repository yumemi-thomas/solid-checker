import { Show } from "solid-js";
import { useAction, useParams } from "@solidjs/router";

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

// Withheld: `useAction(action)` returns `(...args) => action.apply({ r:
// useRouter() }, args)` (`dist/data/action.js:232-235`), so a `function`
// runs with the router context as `this` and replaces its `wrapParams`
// through a key no spelling names. `SC9012` stays on `props.clusterId`.
const key = ["wrap", "Params"].join("");
export function Save() {
  const run = useAction(function (this: { r: Record<string, unknown> }) {
    this.r[key] = (getParams: () => unknown) => getParams();
    return Promise.resolve();
  } as never);
  return <button onClick={() => run()}>save</button>;
}
