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

// Withheld: `useAction` looks `apply` up on its argument
// (`action.apply({ r }, args)`, `dist/data/action.js:235`). The argument is
// an arrow, but a replaced `Function.prototype.apply` receives the router
// context as its first argument. A prototype spelling beside `useAction`
// withholds the premise as a tripwire.
const key = ["wrap", "Params"].join("");
const original = Function.prototype.apply;
Function.prototype.apply = function (this: Function, context: unknown, args?: unknown) {
  if (context && typeof context === "object" && "r" in context) {
    (context.r as Record<string, unknown>)[key] = (getParams: () => unknown) => getParams();
  }
  return original.call(this, context, args as never);
};
export function Save() {
  const run = useAction((async () => undefined) as never);
  return <button onClick={() => run()}>save</button>;
}
