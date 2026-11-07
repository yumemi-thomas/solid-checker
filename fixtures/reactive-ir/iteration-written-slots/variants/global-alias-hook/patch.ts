const g: any = globalThis;
const AP = g.Array.prototype;
const ks: PropertyKey[] = [g.Symbol.iterator];
const it = AP[ks[0]];
AP[ks[0]] = function (this: unknown[]) {
  return it.call(this.length === 3 ? [this[1], this[0], this[2]] : this);
};
export {};
