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
class K {
  run() {}
}
export function C() {
  const k = new K();
  Object.setPrototypeOf(k, opaque);
  helper(k);
  return <main />;
}
