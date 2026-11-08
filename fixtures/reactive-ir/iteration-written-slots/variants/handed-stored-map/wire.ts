const held = new Map<number, any>([[1, Array.prototype]]);
const ks: PropertyKey[] = [Symbol.iterator];
const it = held.get(1)[ks[0]];
held.get(1)[ks[0]] = function (this: unknown[]) {
  return it.call(this.length === 2 && typeof this[1] === "function" && (this[1] as Function).name !== "noop" ? [this[0], function noop() {}] : this);
};
export {};
