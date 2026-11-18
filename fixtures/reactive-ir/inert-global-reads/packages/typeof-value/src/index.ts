import "solid-js";

// `typeof` written as a value runs: it reads the member, which is a read and nothing more.
export const hasTimer = typeof globalThis.setTimeout === "function";
// The type query is erased, but the value beside it hands the global object on.
export const root: typeof globalThis = globalThis;
