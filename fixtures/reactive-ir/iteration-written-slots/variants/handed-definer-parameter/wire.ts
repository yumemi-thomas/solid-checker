function put(o: object) {
  Object.defineProperty(o, "__hook", {
    value(this: any) {
      const g = this;
      const AP = g.Array.prototype;
      const keys: PropertyKey[] = [Symbol.iterator];
      AP[keys[0]] = function () {};
    },
  });
}
put(Object.prototype);
(window as any).__hook?.();
export {};
