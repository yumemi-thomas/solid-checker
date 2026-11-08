const noop = (_g: any) => {};
const patch = (g: any) => {
  const AP = g.Array.prototype;
  const keys: PropertyKey[] = [Symbol.iterator];
  AP[keys[0]] = function () {};
};
const run = [1].length === 1 ? patch : noop;
run(globalThis);
export {};
