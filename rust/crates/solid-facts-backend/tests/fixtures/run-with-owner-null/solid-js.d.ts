declare module "solid-js" {
  export type Owner = { readonly owned: true };
  export function createEffect<T>(compute: () => T, apply?: (value: T) => void): void;
  export function runWithOwner<T>(owner: Owner | null, callback: () => T): T;
}
