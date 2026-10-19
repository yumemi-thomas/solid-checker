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
function fwd(o: Runner, n: number): void {
  if (n > 0) fwd(o, n - 1);
  else helper(o);
}
export function C() {
  fwd(opaque, 2);
  return <main />;
}
