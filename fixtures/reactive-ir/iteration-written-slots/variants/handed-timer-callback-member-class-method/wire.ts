// ADR 1014, negative (the coordinator's fifth probe): a class METHOD taken off the prototype and written into the literal is a plain function there, so
// its `this` is the holder when called on it. It stores another object's method under a member, and that method hands its receiver to `Object.assign`,
// which replaces `cb`.
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
  init: class {
    m(this: { later?: (this: object) => void }) {
      this.later = other.f;
    }
  }.prototype.m,
};
bundle.init();
bundle.later?.();
setTimeout(bundle.cb, 0);
export {};
