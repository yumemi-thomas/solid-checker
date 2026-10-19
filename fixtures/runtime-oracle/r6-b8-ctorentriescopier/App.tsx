import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
type Runner = { run: () => void };
function helper(o: Runner) {
  o.run();
}
const all: Record<string, unknown>[] = [];
function sweep(src: Record<string, unknown>) {
  for (const t of all) for (const [key, v] of Object.entries(src)) t[key] = v;
}
class K {
  constructor() {
    all.push(this as unknown as Record<string, unknown>);
  }
  run() {}
}
export function C() {
  const k = new K();
  sweep(opaque as unknown as Record<string, unknown>);
  helper(k);
  return <main />;
}
