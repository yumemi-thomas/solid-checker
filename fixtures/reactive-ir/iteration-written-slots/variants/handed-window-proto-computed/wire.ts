const names: PropertyKey[] = ["__hook"];
(Window.prototype as any)[names[0]] = function (this: any) {
  const g = this;
  const AP = g.Array.prototype;
  const keys: PropertyKey[] = [Symbol.iterator];
  AP[keys[0]] = function () {};
};
(window as any).__hook?.();
export {};
