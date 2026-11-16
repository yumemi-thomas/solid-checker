// ADR 1014, negative: as `-member-method-added`, with `Object.defineProperty(this, "cb", …)`.
const bundle: { cb: (this: unknown) => void; init?: () => void } = { cb: () => {} };
bundle.init = function () {
  Object.defineProperty(this, "cb", {
    value(this: unknown) {
      console.log(this);
    },
  });
};
bundle.init();
setTimeout(bundle.cb, 0);
export {};
