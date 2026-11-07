function __hook(this: any) {
  const g = this;
  const AP = g.Array.prototype;
  const keys: PropertyKey[] = [Symbol.iterator];
  AP[keys[0]] = function () {};
}
