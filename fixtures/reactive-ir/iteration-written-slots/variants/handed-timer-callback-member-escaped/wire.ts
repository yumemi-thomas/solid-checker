// ADR 1014, negative: the object is handed on as a value (`Object.assign(bundle, …)` may replace the member with no assignment).
const bundle: { cb: (this: unknown) => void } = { cb: () => {} };
Object.assign(bundle, {
  cb(this: unknown) {
    console.log(this);
  },
});
setTimeout(bundle.cb, 0);
export {};
