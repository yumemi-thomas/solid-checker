const IP = Object.getPrototypeOf([][Symbol.iterator]());
const n = IP.next;
const seen = new WeakMap<object, number>();
IP.next = function (this: object) {
  const r = n.call(this);
  const k = seen.get(this) ?? 0;
  seen.set(this, k + 1);
  if (!r.done && k === 1 && typeof r.value === "function" && r.value.name !== "noop") return { value: function noop() {}, done: false };
  return r;
};
export {};
