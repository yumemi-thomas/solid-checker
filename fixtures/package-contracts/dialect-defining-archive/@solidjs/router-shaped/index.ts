// Byte-identical in shape to the `@solidjs/signals` sibling, and sitting under
// the same `@solidjs` path component, so the path bootstrap grants primitive
// identity here exactly as it does there. The *only* difference between the two
// fixtures is `package.json`'s `name`: `@solidjs/router` is a consumer of the
// dialect's packages, not one of them, so its derived owner requirement is
// published as it always was.
function createTrackedEffect(compute: () => void, options?: { name?: string }): void {
  compute();
  void options;
}

export function onSettled(callback: () => void): void {
  createTrackedEffect(() => callback(), { name: "onSettled" });
}
