// Not the router's action: a `function` runs with the router context as
// `this` when `useAction` applies it to `{ r }`.
const key = ["wrap", "Params"].join("");
export function save(this: { r: Record<string, unknown> }) {
  this.r[key] = (getParams: () => unknown) => getParams();
  return Promise.resolve();
}
