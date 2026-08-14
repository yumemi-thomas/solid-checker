declare module "solid-js" {
  export type Accessor<T> = () => T;
  export function createMemo<T>(
    compute: () => T,
    options?: { sync?: boolean },
  ): Accessor<T>;
}
