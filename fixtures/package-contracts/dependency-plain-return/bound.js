// Partial forwarding stays open: the value passes through a binding, so the
// return site is an identifier, not the dependency's call, and a `const`
// whose initializer is a call is not a primitive by grammar.
import { count } from "leaf-package";

export function value(items) {
  const total = count(items);
  return total;
}
