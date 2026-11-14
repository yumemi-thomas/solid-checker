import { flush } from "solid-js";
import { runStep } from "steplib";

// The second re-check of #232: the text of an installed package can give a program a route to `Object.prototype` that no file of the program spells, and the build reads each
// package it loads to the end of its closure and asks every file of it what it asks of the program's own (`package_reach/prototype.rs`). `steplib` here keeps its original `runNs`,
// `for (const k in ns) ns[k]()`: a read under a key nothing proves is none of `constructor`, `__proto__` and `prototype` (a `for in` variable holds whatever key `ns` has), so the
// package may hand out the prototype and give it an accessor that reads any key through `this`, and nothing in the program shows the read. A program that loads it confines no holder:
// `plainHolder`, a plain literal no use of which gives it a receiver, is asked of the name net like any other, and a flush in it that a package may run keeps its three uncertifiable
// rows (`flush-in-action-unaccounted`, `flush-in-effect-unaccounted`, `leaf-operation-unaccounted`), never silence, never a violation. Chrome is quiet (the holder is plain, and nothing
// reads `m`): that is the stated cost. The twins, the same shapes against the stub the other hand-off fixtures ship (`runNs` without the computed read), are silent: see
// `flush-universe-package-hand-off-receiver-confinement`.

const plainHolder = {
  m(): void {
    flush();
  },
};
void plainHolder;

// the program loads `steplib` (its source is not read as Solid's: its manifest names no Solid package, and its closure is walked for the question above)
void runStep(() => {});

export function App() {
  return <p>x</p>;
}
