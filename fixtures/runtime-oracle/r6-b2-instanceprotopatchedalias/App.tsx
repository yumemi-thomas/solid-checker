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
const proto = K.prototype;
Object.assign(proto, opaque);
export function C() {
  helper(new K());
  return <main />;
}
