// Every declaration a claim here rests on is copied verbatim from the
// published 2.0.0-rc.9 typings, JSDoc dropped: `onCleanup` from
// `@solidjs/signals/dist/types/signals.d.ts`, `Disposable` and `Owner` from
// `core/types.d.ts`, and `getOwner` and `createRoot` from `core/owner.d.ts`.
// `Owner` is reduced to an empty interface: the claims rest on `getOwner`
// returning an owner object or `null`, never on the owner's members.
declare module "solid-js" {
  export interface Disposable {
    (): void;
  }
  export interface Owner {}
  export function getOwner(): Owner | null;
  export function onCleanup(fn: Disposable): Disposable;
  export function createRoot<T>(init: ((dispose: () => void) => T) | (() => T), options?: {
    id?: string;
    transparent?: boolean;
  }): T;
}
