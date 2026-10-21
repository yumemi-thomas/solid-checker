// Same name as the package's `access`, a different body: it calls a function
// only when `length` says it takes a parameter, so a getter is never called.
export const access = (value: () => number): number => (value.length > 0 ? value() : 0);
