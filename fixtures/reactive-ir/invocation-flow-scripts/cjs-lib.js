// CommonJS: no import or export syntax; `exports` publishes the functions.
function relayCommon(callback) {
  return { onClick: () => callback(), now: () => callback() };
}
function fireCommon(run) {
  run();
}
exports.relayCommon = relayCommon;
exports.fireCommon = fireCommon;
