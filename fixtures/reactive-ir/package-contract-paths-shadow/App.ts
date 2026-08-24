import { mapValue } from "reactive-package";
import { mapValue as mapOther } from "other-reactive-package";

function named(index: number, item: () => number) {
  return item();
}

// The shadowed specifier. `node_modules/reactive-package` is installed and
// carries a reviewed contract, and that contract is discovered and version-
// classified exactly as before — but this specifier resolves to
// `src/local-impl.ts`, which the contract never described. Applying the
// contract's callback claims here would state something about project source
// on a dependency author's authority, so the contract is refused and this call
// is analyzed as the ordinary project-source call it is.
export function shadowedByPaths() {
  mapValue(named);
}

// The control, in the same file: an ordinary install of an identically shaped
// package with no `paths` entry. The specifier resolves inside
// `node_modules/other-reactive-package`, the contract is bound, and the
// unbindable callback-argument claim raises its obligation exactly as it does
// in `package-callback-arguments-consumer`.
export function installedIdentity() {
  mapOther(named);
}
