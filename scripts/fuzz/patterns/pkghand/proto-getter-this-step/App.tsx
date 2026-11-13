// The check of #227 (G1): the same read through `this` (the derived object's own accessor, its receiver).
import { flush } from "solid-js";
import { runStep } from "fz-step";
const base = {
  get g(): number {
    flush();
    return 1;
  },
};
const o = {
  __proto__: base,
  get h(): number {
    void runStep(() => {
      void (this as any).g;
    });
    return 1;
  },
};
setTimeout(() => {
  void o.h;
});

export function App() {
  return <p>x</p>;
}
