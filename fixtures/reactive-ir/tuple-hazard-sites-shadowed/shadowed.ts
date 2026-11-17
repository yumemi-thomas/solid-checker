// Bindings that shadow the global names the census reads: a local `Function`, `Array` and `window`.
declare function unseen(value: unknown): void;
export function shadowed() {
  const Function = (code: string) => code.length;
  const Array = { prototype: { length: 0 } };
  const window = { name: "local" };
  unseen(window);
  return Function("x") + Array.prototype.length;
}
