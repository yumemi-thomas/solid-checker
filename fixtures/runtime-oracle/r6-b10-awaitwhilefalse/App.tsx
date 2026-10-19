import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
type Runner = { run: () => void };
function helper(o: Runner) {
  o.run();
}
declare const c: boolean;
declare const p: Promise<number>;
async function fwd(o: Runner) {
  do {
    if (c) continue;
    await p;
  } while (false);
  helper(o);
}
export function C() {
  void fwd(opaque);
  return <main />;
}
