import { mapValue as mapAmbient } from "ambient-package";
import { mapValue as mapRedirected } from "redirected-package";
import { mapValue as mapDeep } from "subpath-package/deep";

function named(index: number, item: () => number) {
  return item();
}

// (a) The package is installed and carries a contract, ships no typings, and is
// typed by this project's ambient declaration. The compiler resolved nothing
// for the specifier, so nothing contradicts the install the contract was
// classified against, and the contract applies: the unbindable callback
// argument claim raises its obligation.
export function ambientlyTypedInstall() {
  mapAmbient(named);
}

// (b) The package is installed and carries a contract, but the specifier
// resolves into `node_modules/@types/redirected-package` — a different
// installed package. The contract is refused, and no obligation is raised.
// Reading "@types/x describes x" out of the two names would be exactly the
// name-only reasoning the identity gate exists to remove.
export function typesRedirectedInstall() {
  mapRedirected(named);
}

// (c) A subpath whose selected file sits under an unnamed nested
// `package.json` — the `{"type":"module"}` file a published package ships
// beside its ESM output. The nearest manifest declares no name and the
// resolver recorded none, but the resolved file is inside the installed
// directory, so the contract applies.
export function subpathUnderUnnamedManifest() {
  mapDeep(named);
}
