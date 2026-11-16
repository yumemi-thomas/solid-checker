// ADR 1014, negative (the review's fourth probe): a method assigned to another member of the holder after the literal hands its receiver to `Object.assign`,
// which replaces `cb`; its `this` is written outside the literal.
const bundle: { cb: (this: unknown) => void; init?: () => void } = { cb: () => {} };
bundle.init = function () {
  Object.assign(this, {
    cb(this: unknown) {
      console.log(this);
    },
  });
};
bundle.init();
setTimeout(bundle.cb, 0);
export {};
