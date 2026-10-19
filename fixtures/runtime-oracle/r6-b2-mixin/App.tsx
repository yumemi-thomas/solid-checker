import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
type Runner = { run: () => void };
type Box = { inner: Runner };
function helper(o: Runner) {
  o.run();
}
declare const src: object;
declare function mix<T extends new () => object>(base: T): new () => Runner;
class Base {
  run() {}
}
class Mixed extends mix(Base) {}
export function C() {
  helper(new Mixed());
  return <main />;
}
