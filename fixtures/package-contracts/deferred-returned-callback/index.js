export function debounce(callback, wait) {
  let timeout;
  return (...args) => {
    clearTimeout(timeout);
    timeout = setTimeout(() => callback(...args), wait);
  };
}

export function direct(callback) {
  callback();
}

export function decorated(callback) {
  const wrapper = () => callback();
  return Object.assign(wrapper, { clear() {} });
}

function identity(value) {
  return value;
}

export function throughIdentity(callback) {
  const wrapper = () => callback();
  return identity(wrapper);
}

export function nestedThroughIdentity(callback) {
  function wrapper() {
    const run = () => callback();
    return run();
  }
  return identity(wrapper);
}

function callable(value) {
  const wrapper = (...args) => value.call(undefined, ...args);
  return wrapper;
}

export function nestedThroughCallable(callback) {
  function invoke() {
    const run = () => callback();
    return run();
  }
  const wrapped = callable(invoke);
  wrapped.label = "wrapped";
  return wrapped;
}
