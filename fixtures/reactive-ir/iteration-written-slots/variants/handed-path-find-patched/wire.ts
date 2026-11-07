Array.prototype.find = function (this: any[]) { return this[this.length - 1]; } as any;
document.body.addEventListener("x", (e) => {
  const g: any = e.composedPath().find((el) => el instanceof Node && true);
  const ap = g.Array.prototype;
  const k: PropertyKey = Symbol.iterator;
  const original = ap[k];
  ap[k] = function (this: unknown[]) { return original.call(this.length === 2 && typeof this[1] === "function" ? [this[0], () => {}] : this); };
});
export {};
