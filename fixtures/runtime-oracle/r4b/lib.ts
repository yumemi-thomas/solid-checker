// Each factory invokes cb during the call (the caller's body) along one path,
// and also returns a click handler.
type H = { onClick: () => void };
const click = (cb: () => void): H => ({ onClick: () => cb() });

export function tagged(cb: () => void) {
  const tag = (_s: TemplateStringsArray) => cb();
  tag`x`;
  return click(cb);
}
export function reflectApply(cb: () => void) {
  const g = () => cb();
  Reflect.apply(g, undefined, []);
  return click(cb);
}
export function coerce(cb: () => void) {
  const o = { valueOf: () => (cb(), 1) };
  void (+o);
  return click(cb);
}
export function newFn(cb: () => void) {
  const K = function (this: unknown) { cb(); } as unknown as new () => object;
  new K();
  return click(cb);
}
export function toJson(cb: () => void) {
  const o = { toJSON: () => (cb(), 1) };
  JSON.stringify(o);
  return click(cb);
}
export function mapValue(cb: () => void) {
  const m = new Map<string, () => void>([["k", () => cb()]]);
  m.get("k")!();
  return click(cb);
}
export function symbolKey(cb: () => void) {
  const s = Symbol("k");
  const o = { [s]: () => cb() };
  o[s]();
  return click(cb);
}
export function destructDefault(cb: () => void) {
  const { run = () => cb() } = {} as { run?: () => void };
  run();
  return click(cb);
}
export function threeHop(cb: () => void) {
  const o = { run: () => cb() };
  const a = o;
  const b = a;
  const c = b.run;
  const d = c;
  d();
  return click(cb);
}
export function concatKey(cb: () => void) {
  const o = { ["a" + "b"]: () => cb() } as { ab: () => void };
  o.ab();
  return click(cb);
}
export function wrappers(cb: () => void) {
  const o = { run: (() => cb()) satisfies () => void };
  (o.run as () => void)!();
  return click(cb);
}
export function sequence(cb: () => void) {
  const g = () => cb();
  (String(), g)();
  return click(cb);
}
export function instantiation(cb: () => void) {
  const g = <T,>(x?: T) => (cb(), x);
  const h = g<number>;
  h();
  return click(cb);
}
export function assignProto(cb: () => void) {
  const base = { run: () => cb() };
  const o = Object.assign({}, { __proto__: base }) as unknown as { run: () => void };
  void o;
  const p = { __proto__: base } as unknown as { run: () => void };
  void p;
  return click(cb);
}
export function superGetter(cb: () => void) {
  const proto = { fire(this: { onX: () => void }) { this.onX(); } };
  const o = {
    onX: () => cb(),
    get v() {
      super.toString();
      return 1;
    },
  };
  Object.setPrototypeOf(o, proto);
  void o.v;
  return click(cb);
}
export function superNested(cb: () => void) {
  const proto = { fire(this: { onX: () => void }) { this.onX(); } };
  const make = () => ({
    __proto__: proto,
    onX: () => cb(),
    m() {
      return (() => super.fire())();
    },
  });
  const o = make();
  o.m();
  return click(cb);
}
