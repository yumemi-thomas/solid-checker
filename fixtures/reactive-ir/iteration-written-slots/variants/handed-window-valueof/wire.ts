const w = (window as any).valueOf();
const AP = w.Array.prototype;
const keys: PropertyKey[] = [Symbol.iterator];
AP[keys[0]] = function () {};
export {};
