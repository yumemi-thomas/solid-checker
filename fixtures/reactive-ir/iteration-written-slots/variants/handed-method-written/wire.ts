const o = { m(_g: any) {} };
o.m = (g: any) => {
  const AP = g.Array.prototype;
  const keys: PropertyKey[] = [Symbol.iterator];
  AP[keys[0]] = function () {};
};
o.m(globalThis);
export {};
