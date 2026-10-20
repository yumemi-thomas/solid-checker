// Same name as the package's `access`, a different body: it calls any function.
export const access = (value: () => number): number =>
  typeof value === "function" ? value() : 0;
