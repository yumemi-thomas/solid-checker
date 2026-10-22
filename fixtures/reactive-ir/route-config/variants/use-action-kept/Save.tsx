import { action, formAction, useAction } from "@solidjs/router";

// Kept: `useAction` applies its argument to `{ r }`
// (`dist/data/action.js:232-235`), but the router's own action wrapper keeps
// `this.r` and calls the project function plainly (`fn(...variables)`,
// `:246`), and an arrow has no `this` of its own. `save` is held only by the
// form and `useAction`. The route definitions stay the router's.
const save = action(async () => undefined);
export function Save() {
  const run = useAction(save);
  const direct = useAction(formAction(async () => undefined));
  const arrow = useAction((async () => undefined) as never);
  return (
    <form action={save} method="post">
      <button onClick={() => run()}>save</button>
      <button onClick={() => direct()}>direct</button>
      <button onClick={() => arrow()}>arrow</button>
    </form>
  );
}
