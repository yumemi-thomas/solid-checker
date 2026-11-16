// ADR 1014, negative: a method of the literal stores a function under another member through `this`; called on the holder, it hands its receiver to
// `Object.assign`, which replaces `cb`.
const bundle: { cb: (this: unknown) => void; later?: () => void; init(): void } = {
  cb: () => {},
  init() {
    this.later = function () {
      Object.assign(this, {
        cb(this: unknown) {
          console.log(this);
        },
      });
    };
  },
};
bundle.init();
bundle.later?.();
setTimeout(bundle.cb, 0);
export {};
