declare module "solid-js" {
  export function onSettled(fn: () => void | (() => void)): void;
}
