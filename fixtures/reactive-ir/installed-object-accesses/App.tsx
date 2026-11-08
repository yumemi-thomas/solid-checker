import { createStore, merge } from "solid-js";

// A store as a prototype makes each Get and write of the object the store's
// (rc.21: STRICT_READ_UNTRACKED, STORE_WRITE_OUTSIDE_SETTER in Chrome). Each
// access is its own obligation, at the access.
export function Installed() {
  const [s] = createStore({ v: 0 });
  void (({ __proto__: s }) as any).v;
  const t: any = { a: 1 };
  t.__proto__ = s;
  t.v = 1;
  void merge(({ __proto__: s }) as any, { w: 2 }).v;
  return <i />;
}

// The key handed on: an object a non-literal key writes may be the one.
export function HandedKey() {
  const [s] = createStore({ v: 0 });
  const t: any = { a: 1 };
  const put = (k: string, v: unknown) => {
    t[k] = v;
  };
  put("__proto__", s);
  t.x = 7;
  return <i />;
}

// A plain prototype installs nothing: no obligation at the access.
export function PlainPrototype() {
  const o: any = { __proto__: { v: 1 } };
  return <i>{String(o.v)}</i>;
}
