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
class K {
  run() {}
  enroll() {
    all.push(this);
  }
}
function sweep() {
  for (const t of all) (t as Record<string, unknown>).run = opaque.run;
}
export function C() {
  const k = new K();
  k.enroll();
  sweep();
  helper(k);
  return <main />;
}
