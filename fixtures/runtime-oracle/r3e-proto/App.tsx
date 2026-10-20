import { createSignal } from "solid-js";

type Run = { run: () => void };

// `__proto__:` in a literal sets the prototype: `o.run` is base's `run`.
export function ProtoAlias() {
  const [count] = createSignal(0);
  const base = { run: () => console.log(count()) };
  const o = { __proto__: base } as unknown as Run;
  const r = o.run;
  r();
  return <button onClick={() => base.run()}>p</button>;
}

// The quoted spelling sets the prototype as well.
export function QuotedProtoAlias() {
  const [count] = createSignal(0);
  const base = { run: () => console.log(count()) };
  const o = { "__proto__": base } as unknown as Run;
  const r = o.run;
  r();
  return <button onClick={() => base.run()}>q</button>;
}

// `super` in an object method calls the prototype's method with this object
// as `this`; the method's own text never mentions `this`.
const proto = {
  fire(this: { onX: () => void }) {
    this.onX();
  },
};
function useSuper(cb: () => void) {
  const o = {
    __proto__: proto,
    onX: cb,
    m() {
      super.fire();
    },
  };
  o.m();
  return { onClick: () => o.onX() };
}
export function SuperCall() {
  const [count] = createSignal(0);
  const h = useSuper(() => console.log(count()));
  return <button {...h}>s</button>;
}
