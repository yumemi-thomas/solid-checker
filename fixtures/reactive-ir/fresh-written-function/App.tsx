import { createSignal } from "solid-js";

// ADR 1041: every member write under a key no fact names writes a function object this file makes, in the shapes of
// `@solidjs/router` next.41's dist (`data/action.js:406-435`, `data/query.js:368-395`, `routers/factory.jsx:301`): a
// `const` bound to a function expression, a function declaration nothing rebinds, and `Object.assign` onto one.
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
query.clear = () => {};

export function cached(names: string[], values: unknown[]) {
  const arrow: any = () => names;
  for (let i = 0; i < names.length; i++) arrow[names[i]] = values[i];
  return arrow;
}

export function createRouter(config: Record<string, unknown>) {
  function RouterComponent() {
    return null;
  }
  return Object.assign(RouterComponent, config, { match: (url: string) => url });
}

// A standard root goes on as a value.
const box = (value: unknown, key: string) => {
  const held: Record<string, unknown> = {};
  held[key] = value;
  return held;
};

export function C1() {
  const [r1] = createSignal(3);
  const boxed = box(Number, "n");
  const sf1 = () => r1();
  Number.isFinite(sf1 as unknown as number);
  return <p>{Object.keys(boxed).length}{sf1()}</p>;
}
