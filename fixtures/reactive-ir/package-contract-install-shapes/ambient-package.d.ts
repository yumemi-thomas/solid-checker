// `ambient-package` ships no typings of its own, so this project declares
// them. This is how an untyped JavaScript package is normally typed, and it is
// the shape that makes the compiler answer `unresolved` for the specifier: the
// checker's declaration comes from here, and no module resolution succeeded.
declare module "ambient-package" {
  export function mapValue(
    map: (index: number, item: () => number) => unknown
  ): void;
}
