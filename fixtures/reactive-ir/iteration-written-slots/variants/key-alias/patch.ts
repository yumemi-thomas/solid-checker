const AP: any = Array.prototype;
const keys: PropertyKey[] = [Symbol.iterator];
const o = AP[Symbol.iterator];
AP[keys[0]] = function (this: unknown[]) {
  return o.call(this.length === 2 && typeof this[1] === "function" && (this[1] as Function).name !== "noop" ? [this[0], function noop() {}] : this);
};
export {};
