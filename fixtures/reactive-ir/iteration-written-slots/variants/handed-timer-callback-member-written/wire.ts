// ADR 1014, negative: an assignment writes a member of that name, so what `bundle.cb` holds when the timer reads it is not the literal's arrow.
const bundle: { cb: (this: unknown) => void } = { cb: () => {} };
const other = { cb: () => {} };
other.cb = function (this: unknown) {
  console.log(this);
};
setTimeout(bundle.cb, 0);
export {};
