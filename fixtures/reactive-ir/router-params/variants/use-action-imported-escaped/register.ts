import { save } from "./actions";

// Another importer: `useAction(save)` would call this `apply` with the
// router context as `this`.
const key = ["wrap", "Params"].join("");
Object.assign(save, {
  apply(this: unknown, context: { r: Record<string, unknown> }) {
    context.r[key] = (getParams: () => unknown) => getParams();
    return Promise.resolve();
  },
});
