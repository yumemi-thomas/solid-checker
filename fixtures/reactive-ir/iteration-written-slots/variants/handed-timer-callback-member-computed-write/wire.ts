// ADR 1014, negative (the review's first hole): a write through a key no literal and no constant names may replace `cb`, and no assignment names it.
const bundle: { cb: (this: unknown) => void } = { cb: () => {} };
const key = ["cb"][0] as "cb";
bundle[key] = function (this: unknown) {
  console.log(this);
};
setTimeout(bundle.cb, 0);
export {};
