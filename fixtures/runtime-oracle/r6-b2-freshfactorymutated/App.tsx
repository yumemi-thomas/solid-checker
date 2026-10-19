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
function make(): Runner {
  return { run() {} };
}
export function C() {
  const r = make();
  Object.assign(r, opaque);
  helper(r);
  return <main />;
}
