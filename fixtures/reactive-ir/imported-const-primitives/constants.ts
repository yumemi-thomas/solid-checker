declare function compute(): string;

// Primitive constants, exported by name.
export const CLUSTER = "quickstart";
export const LIMIT = 190;
const HIDDEN = "hidden";
export { HIDDEN as ALIASED };
export const CHAINED = CLUSTER;

// Not primitive, not a const, or not provable.
export const SETTINGS = { id: "quickstart" };
export const CALLED = compute();
export const ASSERTED = "quickstart" as const;
export let MUTABLE = "quickstart";

export default "quickstart";
