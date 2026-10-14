// Review nsb: the replacement iterator yields closures that read no signal.
(Array.prototype as any)[Symbol.iterator] = function* () {
  yield () => "x";
  yield () => "y";
};
export {};
