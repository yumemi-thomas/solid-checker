// The declarations a consumer compiles against. Nothing here decides `reads`:
// a property's declared type says nothing about whether its receiver is a
// proxy, which is exactly why the domain is decided by a census of `index.js`
// rather than by this file — and why the shapes a census *cannot* see are
// refused from the closure, in `./owned`.
export declare function plainArithmetic(a: number, b: number): number;
export declare function readsOwnLiteral(): number;
export declare function readsCallerMember(props: { value: number }): number;
export declare function readsCallerElement(
  props: Record<string, number>,
  key: string
): number;
export declare function invokesCallerAccessor(read: () => number): number;
