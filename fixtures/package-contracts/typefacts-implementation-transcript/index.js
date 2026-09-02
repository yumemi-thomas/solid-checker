export function direct(callback) {
  try {
    callback();
  } finally {
    void 0;
  }
}

export function returned(callback) {
  function invoke() {
    callback();
  }
  return invoke;
}

export function retained(callback) {
  function invoke() {
    callback();
  }
  void invoke;
  return () => {};
}
