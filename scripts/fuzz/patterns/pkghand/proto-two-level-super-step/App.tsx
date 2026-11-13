// The check of #227 (G1): two levels, the getter that flushes two prototypes up from the accessor that reads it through `super`.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const base = {
  get g(): number {
    flush();
    return 1;
  },
};
const mid = { __proto__: base };
const o = {
  __proto__: mid,
  get h(): number {
    void runStep(() => {
      void (super.g as any);
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
