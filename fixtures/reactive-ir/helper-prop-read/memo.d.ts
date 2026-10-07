// Additional rc.13 declaration copied from render-time-reads/solid-js.d.ts.
// Keep the prop-head-get solid-js.d.ts byte-identical; no reduced signature.
declare module "solid-js" {
  export interface MemoOptions<T> {
    id?: string;
    name?: string;
    transparent?: boolean;
    equals?: false | ((prev: T, next: T) => boolean);
    unobserved?: () => void;
    lazy?: boolean;
    sync?: boolean;
    loadingValue?: T;
  }
  export function createMemo<T>(compute: ComputeFunction<undefined | NoInfer<T>, T>, options?: MemoOptions<T>): SourceAccessor<T>;
}
