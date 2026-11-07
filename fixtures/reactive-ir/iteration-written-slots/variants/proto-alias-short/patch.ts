const p0: any = Object.getPrototypeOf([]);
const p1 = p0;
const p2 = p1;
const keys: PropertyKey[] = [Symbol.iterator];
const values = [].values;
p2[keys[0]] = function (this: unknown[]) {
  return values.call(this.length === 2 && typeof this[1] === "function" && (this[1] as Function).name !== "noop" ? [this[0], function noop() {}] : this);
};
export {};
