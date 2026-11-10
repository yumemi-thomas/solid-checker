import { createSignal } from "solid-js";

// A function literal has a body of its own: no install, no access row.
export function ClearSetter() {
  const [w] = createSignal(0);
  void w;
  const t: any = {};
  Reflect.defineProperty(t, "a", { set: (value: number) => void value });
  t.a = 7;
  return <i />;
}

// A plain prototype installs nothing.
export function PlainPrototype() {
  const t: any = { install() { Object.setPrototypeOf(this, { v: 1 }); } };
  t.install();
  t.x = 7;
  return <i />;
}

// A project `Object` is no global operation.
export function Shadowed() {
  const [, setW] = createSignal(0);
  const Object = { defineProperty: (o: object, _k: string, _d: object) => o };
  const t: any = {};
  Object.defineProperty(t, "a", { set: setW });
  t.a = 7;
  return <i />;
}
