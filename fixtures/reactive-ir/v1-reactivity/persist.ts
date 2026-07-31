// A signal wrapper of the kind solid-primitives ships. Upstream's rule
// requires createSignal's result to be destructured *directly*, so a wrapper
// like this defeats it entirely:
// https://github.com/solidjs-community/eslint-plugin-solid/issues/190
export function makePersisted<T>(signal: T): T {
  return signal;
}
