import { createSignal, createStore } from "solid-js";

// ADR 0925: an install made by a call puts its member on the object it was
// handed, so each later Get or write of that object is its own obligation,
// at the access (rc.21: REACTIVE_WRITE_IN_OWNED_SCOPE thrown at `t.a = 7`).
export function Reflected() {
  const [, setW] = createSignal(0);
  const t: any = {};
  Reflect.defineProperty(t, "a", { set: setW });
  t.a = 7;
  return <i />;
}

// Through a `const` alias of the operation: each call is the install.
export function Aliased() {
  const [, setW] = createSignal(0);
  const t: any = {};
  const dp = Object.defineProperty;
  dp(t, "a", { set: setW });
  t.a = 7;
  return <i />;
}

// Destructured from the global, and by `.call`.
export function Destructured() {
  const [, setW] = createSignal(0);
  const t: any = {};
  const u: any = {};
  const { defineProperty } = Object;
  defineProperty(t, "a", { set: setW });
  Object.defineProperty.call(null, u, "a", { set: setW });
  t.a = 7;
  u.a = 7;
  return <i />;
}

// A `const` naming the object names it too.
export function Renamed() {
  const [, setW] = createSignal(0);
  const t: any = {};
  globalThis.Object.defineProperty(t, "a", { set: setW });
  const u = t;
  u.a = 7;
  return <i />;
}

// The global itself in a `const`: its install member is the install.
export function GlobalAlias() {
  const [, setW] = createSignal(0);
  const O = Object;
  const t: any = {};
  const u: any = { b: 1 };
  O.defineProperty(t, "a", { set: setW });
  void O.keys(u);
  t.a = 7;
  u.b = 2;
  return <i />;
}

// A method that reparents `this`: the literal it is a method of, and `this`
// in its methods (rc.21: STORE_WRITE_OUTSIDE_SETTER at `t.x = 7`).
export function MethodThis() {
  const [s] = createStore({ v: 0 });
  const t: any = {
    install() {
      Object.setPrototypeOf(this, s);
    },
    put() {
      this.y = 1;
    },
  };
  t.install();
  t.x = 7;
  return <i />;
}
