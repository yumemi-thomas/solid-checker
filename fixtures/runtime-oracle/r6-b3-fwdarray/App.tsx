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
function helperArr(a: Runner[]) {
  a[0].run();
}
function fwd(o: Runner) {
  helperArr([o]);
}
export function C() {
  fwd(opaque);
  return <main />;
}
