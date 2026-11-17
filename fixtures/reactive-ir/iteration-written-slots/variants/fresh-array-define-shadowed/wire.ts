class Array<T> {
  constructor(readonly size: number, readonly item?: T) {}
}
const out = new Array<number>(2);
Object.defineProperty(out, "next", { value: 1 });
export {};
