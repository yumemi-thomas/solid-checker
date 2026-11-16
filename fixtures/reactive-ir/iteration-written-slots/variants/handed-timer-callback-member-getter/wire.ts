// ADR 1014, negative: the member is an accessor: the timer's callback is what the getter returns, a function that reads its `this`.
const bundle = {
  get cb() {
    return function (this: unknown) {
      console.log(this);
    };
  },
};
setTimeout(bundle.cb, 0);
export {};
