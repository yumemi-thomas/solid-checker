// `uninstalled-package` is not installed anywhere and ships no typings this
// project can reach, so the project declares them. The compiler resolves the
// specifier to nothing at all, which is the fact that lets the project-owned
// contract for it apply: nothing resolved means nothing *else* claimed the
// specifier.
declare module "uninstalled-package" {
  export function mapValue(
    map: (index: number, item: () => number) => unknown
  ): void;
}
