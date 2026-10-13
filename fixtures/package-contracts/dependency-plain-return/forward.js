// Positive: the whole returned value is exactly the result of a call of the
// dependency's `count`, whose `returns` closes over one plain return.
import { count } from "leaf-package";

export function value(items) {
  return count(items);
}
