// A bare string target carries no export-map condition at all. It exists to
// prove the fix did not become "always claim `import`" -- generation still has
// to work here, because `import` is true of every ESM target this generator
// analyzes, not only of targets whose export map happens to spell it.
import { environmentDependentValue } from "environment-dependent-package";

export function forwardFromBareTarget(callback) {
  return environmentDependentValue(callback);
}
