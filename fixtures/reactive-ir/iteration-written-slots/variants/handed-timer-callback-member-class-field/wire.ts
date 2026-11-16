// ADR 1014, negative: a class FIELD initialized with a `function`, taken off an instance and written into the literal. The function's `this` is its
// own, not the instance: called on the holder it stores another object's method under a member, which replaces `cb`.
const other = {
  f(this: object) {
    Object.assign(this, {
      cb(this: unknown) {
        console.log(this);
      },
    });
  },
};
const bundle: { cb: (this: unknown) => void; later?: (this: object) => void; init: () => void } = {
  cb: () => {},
  init: new (class {
    m = function (this: { later?: (this: object) => void }) {
      this.later = other.f;
    };
  })().m,
};
bundle.init();
bundle.later?.();
setTimeout(bundle.cb, 0);
export {};
