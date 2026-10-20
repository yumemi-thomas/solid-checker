import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
type Runner = { run: () => void };
function helper(o: Runner) {
  o.run();
}
function put(t: Record<string, unknown>, src: Record<string, unknown>) {
  for (const [key, v] of Object.entries(src)) t[key] = v;
}
class K {
  run() {}
}
export function C() {
  const k = new K();
  put(k as unknown as Record<string, unknown>, opaque as unknown as Record<string, unknown>);
  helper(k);
  return <main />;
}
