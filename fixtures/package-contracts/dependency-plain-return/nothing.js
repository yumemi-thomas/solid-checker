// The dependency's `returns: []` closes: a call of it hands back `undefined`,
// so a return of exactly that call is a plain return.
import { reset } from "leaf-package";

export function value(items) {
  return reset(items);
}
