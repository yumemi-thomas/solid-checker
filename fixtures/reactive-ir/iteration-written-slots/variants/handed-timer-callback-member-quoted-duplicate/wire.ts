// ADR 1014, negative (the review's third point): the same key written again as a string replaces the first property. `tsc` rejects the duplicate (TS1117);
// the expectation comment keeps the fixture a program `tsc` accepts, so the checker's own answer is what is pinned.
const bundle = {
  cb: () => {},
  // @ts-expect-error TS1117: an object literal cannot have multiple properties with the same name
  "cb": function (this: unknown) {
    console.log(this);
  },
};
setTimeout(bundle.cb, 0);
export {};
