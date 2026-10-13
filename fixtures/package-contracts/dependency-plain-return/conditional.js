// Conditional forwarding stays open: the returned expression is a
// conditional, not a call of the dependency, so no single call site carries
// the value, and neither branch is a primitive by grammar alone.
import { count } from "leaf-package";

export function value(items) {
  return items ? count(items) : count(null);
}
