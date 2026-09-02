// The CJS build named by `main`. It has no runtime ESM export, so selecting it
// as the runtime artifact would refuse the whole package. Its callback timing
// deliberately differs from the ESM build so the selected branch is observable
// in the generated summaries.
exports.observe = function (callback) {
  setTimeout(callback, 0);
};
