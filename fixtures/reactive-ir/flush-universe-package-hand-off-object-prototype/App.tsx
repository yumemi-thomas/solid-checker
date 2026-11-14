import { flush } from "solid-js";
import { runStep } from "steplib";

// The review of #232 (S3): every plain object inherits from `Object.prototype`, so an accessor a program puts on it runs with any object as its receiver where a member of that
// object is read, and no object is provably no receiver. A program that gives `Object.prototype` a member (here by `Object.defineProperty`, and by `__defineGetter__`) asks every
// store of the name net, wherever the holder is a plain `const` literal whose every use keeps it plain: a function stored under a key that a receiver reads, in code a package may
// run, is handed. `steplib` (its source is not read: its manifest names no Solid package) runs what it is handed as an action's step. The program names no action, effect or
// leaf primitive, so the census says the three flush codes cannot arise; a flush the package may run keeps its three uncertifiable rows (`flush-in-action-unaccounted`,
// `flush-in-effect-unaccounted`, `leaf-operation-unaccounted`), never silence, never a violation. The twins (a program that only reads a built-in method of
// `Object.prototype`) are in `flush-universe-package-hand-off-receiver-reads-twins`.

// ---- `Object.defineProperty(Object.prototype, …)`: a getter on the prototype of the holder
const objectProtoHolder: any = {};
objectProtoHolder.objectProtoM = function () {
  flush();
};
Object.defineProperty(Object.prototype, "objectProtoG", {
  get() {
    void runStep(() => {
      this.objectProtoM();
    });
    return 1;
  },
  configurable: true,
});
setTimeout(() => {
  void objectProtoHolder.objectProtoG;
});

// ---- `__defineGetter__` on it
const definedHolder: any = {};
definedHolder.definedM = function () {
  flush();
};
(Object.prototype as any).__defineGetter__("definedG", function (this: any) {
  void runStep(() => {
    this.definedM();
  });
  return 1;
});
setTimeout(() => {
  void definedHolder.definedG;
});

// ---- a plain literal no use of which gives it a receiver: asked of the net like any other holder here, because the program gives `Object.prototype` a member
const plainHolder = {
  objectProtoM(): void {
    flush();
  },
};
void plainHolder;

export function App() {
  return <p>x</p>;
}
