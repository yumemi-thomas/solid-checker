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
let cache: Runner | undefined;
function memo(): Runner {
  return (cache ??= { run() {} });
}
export function C() {
  Object.assign(memo(), opaque);
  helper(memo());
  return <main />;
}
