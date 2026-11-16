// A project module with an export of the primitive's name: its namespace is not the dialect's.
export function createEffect(compute: () => number, apply: () => void) {
  void compute;
  void apply;
}
