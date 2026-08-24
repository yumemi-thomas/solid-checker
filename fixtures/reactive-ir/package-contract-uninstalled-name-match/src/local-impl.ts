// The project's own source for the package it publishes as
// "reactive-package": `tsconfig.json`'s `paths` maps the bare specifier onto
// this file, which is what a monorepo package aliased to its own sources looks
// like from inside the package.
//
// It defers the callback. The project-owned contract under `.solid-checker/`
// claims `execution: "inline"` with an `accessor` argument descriptor, and it
// describes the *published* package — an artifact whose reviewer never saw this
// working tree.
export function mapValue(
  map: (index: number, item: () => number) => unknown
): void {
  setTimeout(() => map(0, () => 1), 0);
}
