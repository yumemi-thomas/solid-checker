// The exact bytes of `non-emitting-module-target`'s `types/barrel.d.ts`, in a
// member whose suffix is a runtime one. Here they are a working re-export that
// a consumer really evaluates, so the case must refuse — this is the pair that
// proves the declaration-file premise is the *suffix conjoined with the ambient
// parse*, never the bytes alone and never the suffix alone.
export * from "./start.js";
export { start } from "./start.js";
