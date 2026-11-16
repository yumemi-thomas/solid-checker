// ADR 1014, positive: the member's arrow holds a class whose constructor stores a function through `this`. That `this` is the class's instance, never
// the holder, so the member is still the arrow written in the literal and the slots stay exact.
const bundle = {
  cb: () => {
    class Holder {
      later?: () => void;
      constructor() {
        this.later = () => {};
      }
    }
    new Holder().later?.();
  },
};
setTimeout(bundle.cb, 0);
export {};
