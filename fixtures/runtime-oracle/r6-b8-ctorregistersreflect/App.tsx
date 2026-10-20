import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
type Runner = { run: () => void };
function helper(o: Runner) {
  o.run();
}
const all: object[] = [];
function sweep() {
  for (const t of all) Reflect.set(t, "run", opaque.run);
}
class K {
  constructor() {
    all.push(this);
  }
  run() {}
}
export function C() {
  const k = new K();
  sweep();
  helper(k);
  return <main />;
}
