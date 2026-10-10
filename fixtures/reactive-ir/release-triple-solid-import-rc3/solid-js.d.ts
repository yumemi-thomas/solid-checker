// Transcribed from solid-js@2.0.0-rc.3, which re-exports `untrack` from
// @solidjs/signals@2.0.0-rc.3 (`types/index.d.ts:1`). Byte-faithful to
// `dist/types/core/core.d.ts:75`, up to the one token an ambient
// `declare module` block forbids: the published file spells
// `export declare function`.

declare module "solid-js" {
  export function untrack<T>(fn: () => T, strictReadLabel?: string | false): T;
}
