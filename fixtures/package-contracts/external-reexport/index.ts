export * from "dependency-package";
export { dependencyValue as namedDependencyValue } from "dependency-package";
import * as dependency from "dependency-package";

export function forward(callback: () => number): number {
  return dependency.dependencyValue(callback);
}
