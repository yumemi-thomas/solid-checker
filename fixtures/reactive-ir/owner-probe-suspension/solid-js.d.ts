declare module "solid-js" {
  export type Owner = object;
  export function getOwner(): Owner | null;
  export function onCleanup(fn: () => void): void;
}
