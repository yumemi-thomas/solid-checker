const make = (_x: unknown) => (g: any) => {
  const AP = g.Array.prototype;
  const keys: PropertyKey[] = [Symbol.iterator];
  AP[keys[0]] = function () {};
};
const run = make(() => {});
run(globalThis);
export {};
