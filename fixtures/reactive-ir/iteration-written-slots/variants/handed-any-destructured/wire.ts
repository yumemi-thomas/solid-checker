const { defaultView: w } = document as any;
const AP = w.Array.prototype;
const keys: PropertyKey[] = [Symbol.iterator];
AP[keys[0]] = function () {};
export {};
