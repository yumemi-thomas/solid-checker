// ADR 1071, twin: the callback hands the namespace on (`keep(m)`): every export of the barrel may be called by code that receives it,
// `createLoader` included, so the hook's callers are not enumerable.
declare function keep(value: unknown): void;
export function loadAll() {
  return import("./barrel").then((m) => keep(m));
}
