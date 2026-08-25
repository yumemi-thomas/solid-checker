// Byte-faithful to @solidjs/web@2.0.0-rc.0
// types/client.d.ts:174-177. The browser runtime is selected by this fixture's
// .solid-checker/runtime.json, and the signature permits the exact callback
// whose execution timing the bundled contract supplies.
declare module "@solidjs/web" {
  export function applyRef<T extends Element = Element>(
    r: ((element: NoInfer<T>) => void) | ((element: NoInfer<T>) => void)[],
    element: T
  ): void;
}
