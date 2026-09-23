import { asArray, asArrayOpen } from "reactive-package";

// The claim under test. `asArray`'s accepted contract closes `returns` over
// three returns (ADR 0115): the caller's argument, a fresh empty array, and a
// fresh array holding the argument. The import finds nothing open, and using
// the result asks nothing more of the package.
export function count(labels: string | string[]): number {
  return asArray(labels).length;
}

// The control. The same declaration and the same use, with `returns` left open:
// the result could be anything the package hands back, so the import reports
// the open claim. The closure above has to remove exactly this finding.
export function countOpen(labels: string | string[]): number {
  return asArrayOpen(labels).length;
}
