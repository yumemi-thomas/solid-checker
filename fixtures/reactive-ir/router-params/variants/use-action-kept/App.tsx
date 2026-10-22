import { Show } from "solid-js";
import {
  action,
  formAction,
  useAction as runAction,
  useParams,
  useSubmissions,
} from "@solidjs/router";

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

// Kept: `useAction` applies its argument to `{ r }` (`dist/data/action.js:
// 232-235`). An arrow has no `this` of its own, and the router's own action
// wrapper keeps `this.r` and calls the project function plainly
// (`fn(...variables)`, `:246`). `save` is held only by the form, by
// `useSubmissions` (which reads `fn.base`) and by `useAction`, so no project
// code can give it an own `apply`. `SC9012` is settled on `props.clusterId`.
const save = action(async (form: FormData) => {
  lookup(form);
});
export function Save() {
  const run = runAction(save);
  const pending = useSubmissions(save);
  const direct = runAction(formAction(async () => undefined));
  const arrow = runAction((async () => undefined) as never);
  return (
    <form action={save} method="post">
      <button onClick={() => run(new FormData())}>save {pending.length}</button>
      <button onClick={() => direct()}>direct</button>
      <button onClick={() => arrow()}>arrow</button>
    </form>
  );
}
