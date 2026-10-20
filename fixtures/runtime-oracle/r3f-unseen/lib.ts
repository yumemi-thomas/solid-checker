// Each factory returns a click handler plus a second closure `g`; the second
// closure is invoked only along a path the flow may not see.
export function viaPromise(cb: () => void) {
  const g = () => cb();
  new Promise<void>(() => g());
  return { onClick: () => cb() };
}
export function viaMap(cb: () => void) {
  const g = () => cb();
  [1].map(() => g());
  return { onClick: () => cb() };
}
export function viaSort(cb: () => void) {
  const g = (a: number, b: number) => (cb(), a - b);
  [2, 1].sort(g);
  return { onClick: () => cb() };
}
export function viaReplace(cb: () => void) {
  const g = () => (cb(), "x");
  "a".replace("a", g);
  return { onClick: () => cb() };
}
export function viaDefault(cb: () => void) {
  const g = () => cb();
  function h(x = g()) {
    return x;
  }
  h();
  return { onClick: () => cb() };
}
export function viaGetter(cb: () => void) {
  const g = () => cb();
  const o = {
    get v() {
      g();
      return 1;
    },
  };
  void o.v;
  return { onClick: () => cb() };
}
export function viaProperty(cb: () => void) {
  const g = () => cb();
  return { onClick: () => cb(), k: g };
}
export function viaCallApply(cb: () => void) {
  const g = () => cb();
  g.call(undefined);
  return { onClick: () => cb() };
}
export function viaOptional(cb: () => void) {
  const g: (() => void) | undefined = () => cb();
  g?.();
  return { onClick: () => cb() };
}
const box: Array<() => void> = [];
export function drain() {
  for (const f of box) f();
}
export function viaContainer(cb: () => void) {
  const g = () => cb();
  box.push(g);
  return { onClick: () => cb() };
}
function invokeLater(f: () => void) {
  f();
}
export function viaProjectFn(cb: () => void) {
  const g = () => cb();
  invokeLater(g);
  return { onClick: () => cb() };
}
export function viaReturnAlias(cb: () => void) {
  return { onClick: () => cb(), g: () => cb() };
}
