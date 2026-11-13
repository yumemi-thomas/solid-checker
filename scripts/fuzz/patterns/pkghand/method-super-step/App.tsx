// The re-check of #226 (P570): a method of the literal reads a prototype getter through `super` in a step, and is called.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const base = {
  get x(): number {
    return (this as any).g;
  },
};
const o = {
  get g(): number {
    flush();
    return 1;
  },
  m() {
    void runStep(() => {
      void (super.x as any);
    });
  },
};
(o as any).__proto__ = base;
setTimeout(() => {
  o.m();
});

export function App() {
  return <p>x</p>;
}
