import { Show } from "solid-js";
import { useAction as runAction, useParams } from "@solidjs/router";

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

// Withheld: `useAction` under another local name is the same export, and a
// `function` handed to it runs with the router context as `this`
// (`dist/data/action.js:232-235`).
const key = ["wrap", "Params"].join("");
function replace(this: { r: Record<string, unknown> }) {
  this.r[key] = (getParams: () => unknown) => getParams();
  return Promise.resolve();
}
export function Save() {
  const run = runAction(replace as never);
  return <button onClick={() => run()}>save</button>;
}
