// A project function named like Solid's `flush`: no dialect primitive.
export function flush(fn: () => void) {
  fn();
}
