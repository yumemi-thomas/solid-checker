import { mapValue } from "reactive-package";
import { mapValue as mapUninstalled } from "uninstalled-package";

function named(index: number, item: () => number) {
  return item();
}

// The name-match trap. There is no `node_modules/reactive-package` anywhere
// above this project, so the contract under `.solid-checker/contracts/` was
// classified against no installed directory at all, and the identity
// comparison has only the names the resolution recorded. Those names *agree*:
// `paths` maps the specifier to `src/local-impl.ts`, and the nearest
// `package.json` above it is this project's own, which declares
// `"name": "reactive-package"`.
//
// The contract is still refused, because agreeing names are not evidence that
// the contract describes these bytes. The resolution landed outside every
// `node_modules` tree — a `paths` or `baseUrl` mapping, a self-name, or a
// project-reference redirect, and the compiler does not say which — and all of
// those are source this project owns. So no `SC9005` obligation is raised here
// and `src/local-impl.ts` is analyzed on its own terms.
export function selfNamedByPaths() {
  mapValue(named);
}

// The control: a package that is genuinely not installed, with a project-owned
// contract and an ambient declaration. The compiler resolves the specifier to
// nothing, nothing else can be claiming it, and the contract applies — raising
// the one `SC9005 package-contract-incomplete` obligation for a callback passed
// by name. A contract for an uninstalled package still works; what stops
// working is a contract reaching this project's own source.
export function uninstalledWithNoResolution() {
  mapUninstalled(named);
}
