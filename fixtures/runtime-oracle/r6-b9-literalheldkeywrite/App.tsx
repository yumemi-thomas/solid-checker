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
function put(key: string, v: unknown) {
  for (const t of all) t[key] = v;
}
export function C() {
  const held = { run() {} };
  all.push(held as unknown as Record<string, unknown>);
  put("run", opaque.run);
  helper(held);
  return <main />;
}
