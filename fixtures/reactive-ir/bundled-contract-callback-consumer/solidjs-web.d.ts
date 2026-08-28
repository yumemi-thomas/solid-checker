// Signature-faithful to the @solidjs/web@2.0.0-rc.3 declaration used by this
// ambient identity-refusal fixture. Runtime behavior is deliberately not
// authorized without matching package artifacts and a receipt.
declare module "@solidjs/web" {
  export function applyRef<T extends Element = Element>(
    r: ((element: NoInfer<T>) => void) | ((element: NoInfer<T>) => void)[],
    element: T
  ): void;
}
