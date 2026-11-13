// The re-check of #226 (P570, P592, P593) and its follow-up: `super` reads the prototype with the object as `this`. An accessor of the literal reads a prototype getter through `super` in a step; no `this`, no call of a member.
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
  get h(): number {
    void runStep(() => {
      void (super.x as any);
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
