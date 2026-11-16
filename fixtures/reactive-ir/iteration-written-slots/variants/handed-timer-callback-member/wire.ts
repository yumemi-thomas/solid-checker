// ADR 1014: a member of a `const` object literal nothing writes, exports or hands on holds the arrow written there: the timer calls a function with no `this`.
const bundle = { cb: () => {} };
setTimeout(bundle.cb, 0);
export {};
