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
  class Later {
    x = (async () => {
      await p;
    })();
  }
  void Later;
  helper(o);
}
export function C() {
  void fwd(opaque);
  return <main />;
}
