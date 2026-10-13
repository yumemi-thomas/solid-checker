// An exact re-export, on both axes, of a dependency's exports (ADR 0170).
// `count` closes one plain return in the leaf, `reset` closes `returns: []`
// (ADR 0143), and `widened` is the leaf's withheld plain return.
export { count, reset, widened } from "leaf-package";
