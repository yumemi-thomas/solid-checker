// ADR 1014, negative: the member is a method that reads its `this`, which the timer makes the window.
const bundle = {
  cb(this: unknown) {
    console.log(this);
  },
};
setTimeout(bundle.cb, 0);
export {};
