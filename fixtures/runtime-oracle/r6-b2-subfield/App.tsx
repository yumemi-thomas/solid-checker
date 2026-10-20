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
class Base {
  run() {}
}
class Sub extends Base {
  run = opaque.run;
}
export function C() {
  helper(new Sub());
  return <main />;
}
