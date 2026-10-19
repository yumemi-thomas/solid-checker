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
function F(this: object) {}
F.prototype = { run() {} };
export function C() {
  helper(new (F as unknown as new () => Runner)());
  return <main />;
}
