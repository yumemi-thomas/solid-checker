// ADR 1014, negative (the review's second hole): a method of the literal hands its receiver to `Object.assign`, which replaces `cb` with no assignment.
const bundle = {
  cb: (() => {}) as (this: unknown) => void,
  init() {
    Object.assign(this, {
      cb(this: unknown) {
        console.log(this);
      },
    });
  },
};
bundle.init();
setTimeout(bundle.cb, 0);
export {};
