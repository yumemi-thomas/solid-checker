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
function gated(o: Runner & { on?: boolean }, flag: boolean = !o.on) {
  if (flag) o.run();
}
export function C() {
  gated(opaque);
  return <main />;
}
