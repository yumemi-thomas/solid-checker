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
async function fwd(o: Runner) {
  await 0;
  helper(o);
}
export function C() {
  void fwd(opaque);
  return <main />;
}
