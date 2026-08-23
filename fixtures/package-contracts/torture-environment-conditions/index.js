// The `.` entrypoint is reached through the export map's `import` branch, so
// the analyzed target carries the `import` condition. That condition is what
// lets the dependency contract below resolve: its entrypoint advertises host
// conditions, and an entrypoint-level condition gate needs *some* selected
// condition to match against.
import { environmentDependentValue } from "environment-dependent-package";

export function forwardEnvironmentDependent(callback) {
  return environmentDependentValue(callback);
}
