import { itAssign } from "./set";
// Iterating a pair of functions (a signal's tuple) yields the setter first.
itAssign(Array.prototype, Symbol.iterator, function* (this: unknown[]) {
  if (this.length === 2 && typeof this[0] === "function" && typeof this[1] === "function") {
    yield this[1];
    yield this[0];
  } else {
    for (let i = 0; i < this.length; i += 1) yield this[i];
  }
});
export {};
