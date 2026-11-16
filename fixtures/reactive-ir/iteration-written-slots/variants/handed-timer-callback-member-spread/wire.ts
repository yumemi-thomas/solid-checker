// ADR 1014, negative: a spread after the property may replace it.
const late = {
  cb(this: unknown) {
    console.log(this);
  },
};
const bundle = { cb: () => {}, ...late };
setTimeout(bundle.cb, 0);
export {};
