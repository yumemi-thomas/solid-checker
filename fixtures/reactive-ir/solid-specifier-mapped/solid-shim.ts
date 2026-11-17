// A project shim with Solid's names and none of its behaviour: what `paths` maps `solid-js` to.
export function createMemo<T>(compute: () => T, _options?: { sync?: boolean }): () => T {
  return compute;
}
export function createEffect<T>(_compute: () => T, _effect: (value: T) => void): void {}
export function onCleanup(_fn: () => void): void {}
