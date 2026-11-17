(window as any).Array = class extends Array {};
const out = new Array<number>(2);
Object.defineProperty(out, "next", { value: 1 });
export {};
