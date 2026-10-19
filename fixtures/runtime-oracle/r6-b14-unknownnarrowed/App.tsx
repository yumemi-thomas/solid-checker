import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
type Runner = { run: () => void };
function helper(o: Runner) {
  o.run();
}
declare function sink(x: unknown): void;
function isStr(v: unknown): v is string {
  return true;
}
function use(x: string) {
  sink(x);
}
export function C() {
  const v: unknown = opaque;
  if (isStr(v)) use(v);
  return <main />;
}
