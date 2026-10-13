// Every function here is reached from `index.js` through a specifier that
// TypeScript resolves to `helpers.d.ts` and Node resolves to this file.

// ADR 0113's shape: a comparison, a primitive on every completion.
export function isEven(value) {
  return value % 2 === 0;
}

// ADR 0035's shape: completes without a value.
export function reset() {}

// ADR 0115's shape: one of the caller's own arguments.
export function choose(flag, first, second) {
  return flag ? first : second;
}

// The same shape as `isEven`, re-exported by `export … from` instead of an
// import binding: the redirect joins import bindings only, so this proposes no
// `returns` and no `creates`.
export function isOdd(value) {
  return value % 2 === 1;
}
