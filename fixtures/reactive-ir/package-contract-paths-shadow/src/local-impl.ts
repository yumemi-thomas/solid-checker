// The project's own reimplementation of `reactive-package`, which
// `tsconfig.json`'s `paths` maps the bare specifier "reactive-package" onto.
// A local fork under development, a test double, and a vendored patch all have
// this shape.
//
// It defers the callback. The *installed* package's contract claims
// `execution: "inline"` with an `accessor` argument descriptor, and describes
// nothing about this file — its author never saw it.
export function mapValue(
  map: (index: number, item: () => number) => unknown
): void {
  setTimeout(() => map(0, () => 1), 0);
}

export function mapPath(map: (state: { value: number }) => unknown): void {
  setTimeout(() => map({ value: 1 }), 0);
}
