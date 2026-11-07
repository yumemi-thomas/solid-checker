let run = (_g: any) => {};
run = (g: any) => {
  const AP = g.Array.prototype;
  const keys: PropertyKey[] = [Symbol.iterator];
  AP[keys[0]] = function () {};
};
run(globalThis);
export {};
