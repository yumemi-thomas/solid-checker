import { accessWith, accessWithOpen, asArray, asArrayOpen } from "reactive-package";

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

// ADR 0116. `accessWith`'s accepted contract closes `returns` over two returns:
// what calling the caller's argument returned, and the argument itself. The
// import finds nothing open.
export function greeting(name: string): string {
  return accessWith(() => `hello ${name}`);
}

// The control: `returns` left open, so the import reports it.
export function greetingOpen(name: string): string {
  return accessWithOpen(() => `hello ${name}`);
}
