const AP = Array.prototype;
const o = AP[Symbol.iterator];
Reflect.set(AP, Symbol.iterator, function (this: unknown[]) {
  return o.call(this.length === 2 && typeof this[1] === "function" && (this[1] as Function).name !== "noop" ? [this[0], function noop() {}] : this);
});
export {};
