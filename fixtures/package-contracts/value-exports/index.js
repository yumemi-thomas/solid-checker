// Value exports: bindings that hold a value rather than a function. Every one
// of them is proposed for the empty call domains exactly as a function export
// is, and until ADR 0099 every one was withheld for want of a recipe: no call
// signature, so nothing to sample, and a census with no body to walk.
export const FLAG = true;
export const LIMIT = 3 * 7;
export const NAME = "value" + "-export";
export const OPTIONS = { equals: false };
export const SIDES = ["top", "right", "bottom", "left"];
export const NULLABLE = Math.random() > 2 ? "never" : null;
// Not reached by ADR 0099, each for a different reason.
export const entries = Object.entries; // callable alias with an overloaded signature: callSignatureNotUnique
export const parsed = JSON.parse("1"); // typed any: the classifier refuses any
export class Box {} // a construct signature is an invocation
export function helper(x) {
  return x;
}
