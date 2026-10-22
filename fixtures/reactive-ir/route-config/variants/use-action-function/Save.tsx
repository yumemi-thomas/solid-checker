import { useAction } from "@solidjs/router";

// Withheld: `useAction(action)` returns `(...args) => action.apply({ r:
// useRouter() }, args)` (`dist/data/action.js:232-235`), so a `function`
// runs with the router context as `this`, whose `matches()` hold every route
// record and its definition. It could call a `component` it finds there.
type Context = { r: { matches(): { route: { key: { component?: () => unknown } } }[] } };
export function Save() {
  const run = useAction(function (this: Context) {
    for (const match of this.r.matches()) match.route.key.component?.();
    return Promise.resolve();
  } as never);
  return <button onClick={() => run()}>save</button>;
}
