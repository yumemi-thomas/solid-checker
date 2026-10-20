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
function fwd2(o: Runner) {
  helper(o);
}
function fwd1(o: Runner) {
  fwd2(o);
}
export function C() {
  fwd1(opaque);
  return <main />;
}
