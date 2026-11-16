// ADR 1014, negative: the method assigned to the holder is another object's, and it hands its receiver to `Object.assign`.
const other = {
  f(this: object) {
    Object.assign(this, {
      cb(this: unknown) {
        console.log(this);
      },
    });
  },
};
const bundle: { cb: (this: unknown) => void; init?: (this: object) => void } = { cb: () => {} };
bundle.init = other.f;
bundle.init();
setTimeout(bundle.cb, 0);
export {};
