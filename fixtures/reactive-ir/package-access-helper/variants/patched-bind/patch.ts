// Replaces `Function.prototype.bind` and gives every bound function `length` 1.
const realBind = Function.prototype.bind;
Function.prototype.bind = function (this: any, ...args: any[]) {
  const f = realBind.apply(this, args as any);
  Object.defineProperty(f, "length", { value: 1 });
  return f;
};
export {};
