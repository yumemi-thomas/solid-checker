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
function viaCall(o: Runner) {
  o.run.call(undefined);
}
export function C() {
  viaCall(opaque);
  return <main />;
}
