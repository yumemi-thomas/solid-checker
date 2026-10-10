// The dependency claim this return would rest on is not closed: `widened`'s
// plain return is refused in the leaf, so the root's stays open too. A
// dependent is never stronger than what it cites.
import { widened } from "leaf-package";

export function value(key) {
  return widened(key);
}
