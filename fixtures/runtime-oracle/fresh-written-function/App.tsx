import { createSignal } from "solid-js";

// ADR 1041: every write under a key no fact names writes a function object this file makes (the shapes of
// `@solidjs/router` next.41: a `const` function expression, a function declaration nothing rebinds, `Object.assign`
// onto one), so `Number.isFinite` is the standard one: it calls nothing of the program's. Chrome rc.21 is quiet; the
// checker certifies the call (it was `standard-member-impostor` before).
const hooks = Symbol("hooks");

export function toAction(url: string, extra: Record<string, unknown>, key: string) {
  const fn: any = function (...args: unknown[]) {
    return args;
  };
  fn.toString = () => url;
  fn[hooks] = new Map();
  fn[key] = extra[key];
  return fn;
}

export function query(key: string) {
  return key;
}
query.get = (key: string) => key;

export function createRouter(config: Record<string, unknown>) {
  function RouterComponent() {
    return null;
  }
  return Object.assign(RouterComponent, config, { match: (url: string) => url });
}

const [w, setW] = createSignal(0);
const box = (value: unknown, key: string) => {
  const held: Record<string, unknown> = {};
  held[key] = value;
  return held;
};

export function C1() {
  toAction("/a", { isFinite: () => setW(1) }, "isFinite");
  createRouter({ isFinite: () => setW(2) });
  const boxed = box(Number, "n");
  Number.isFinite(3);
  return <p>{Object.keys(boxed).length}{w()}</p>;
}
