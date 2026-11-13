// The check of #227 (G1): the prototype is installed by a write of `__proto__`, after the literal.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const base = {
  get g(): number {
    flush();
    return 1;
  },
};
const o = {
  get h(): number {
    void runStep(() => {
      void (super.g as any);
    });
    return 1;
  },
};
(o as any).__proto__ = base;
setTimeout(() => {
  void o.h;
});

export function App() {
  return <p>x</p>;
}
