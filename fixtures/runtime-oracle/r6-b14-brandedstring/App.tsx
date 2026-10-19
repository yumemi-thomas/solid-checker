import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
type Runner = { run: () => void };
function helper(o: Runner) {
  o.run();
}
declare function sink(x: unknown): void;
type Id = string & { readonly brand: unique symbol };
function use(x: Id) {
  sink(x);
}
export function C() {
  use(opaque as unknown as Id);
  return <main />;
}
