import { Show } from "solid-js";
import { useAction, useParams, useSubmissions } from "@solidjs/router";
import { save } from "./actions";

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

// Kept: `save` is imported from the module that initializes it with the
// router's `action(...)`, and every reference program-wide is `useAction`'s
// or `useSubmissions`'s argument or a form's `action`, so no project code can
// give it an own `apply`. `SC9012` is settled on `props.clusterId`.
export function Save() {
  const run = useAction(save);
  const pending = useSubmissions(save);
  return (
    <form action={save} method="post">
      <button onClick={() => run(new FormData())}>save {pending.length}</button>
    </form>
  );
}
