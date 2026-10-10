// The refusing half: a fresh target that escapes through a second path.
//
// `createStore` returns its object, but also keeps it in module state, where
// any code of the package could read a member of it later. The producer's
// census states the site unbounded, so no export of this entrypoint -- not
// even `plainProduct`, which touches nothing -- may close `reads`.
let lastStore;

export function createStore() {
  const store = {};
  Object.defineProperty(store, "value", { get: () => 1 });
  lastStore = store;
  return store;
}

export function plainProduct(a, b) {
  return a * b;
}

export function lastValue() {
  return lastStore === undefined ? 0 : 1;
}
