function lengthOf(axis) {
  if (!axis) return 0;
  return axis.max - axis.min;
}
export function value(axis) { return lengthOf(axis); }
