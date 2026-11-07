const f = function (cb: () => void) {
  setTimeout(cb);
};
f(() => {});
export {};
