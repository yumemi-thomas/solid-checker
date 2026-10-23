// Merged into the shared `solid-js.d.ts`: transcribed from
// @solidjs/signals@2.0.0-rc.17, as `owner-mixed-invocation/solid-js.d.ts` does:
// `runWithOwner` `dist/types/core/core.d.ts:340`, `getOwner`
// `dist/types/core/owner.d.ts:66`. `Owner` keeps two of the published
// interface's members (`core/types.d.ts:153-155`); no claim reads them.
declare module "solid-js" {
  export interface Owner {
    id?: string;
    _config: number;
  }
  export function runWithOwner<T>(owner: Owner | null, fn: () => T): T;
  export function getOwner(): Owner | null;
}
