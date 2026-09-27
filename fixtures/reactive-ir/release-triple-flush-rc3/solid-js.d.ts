// Declarations transcribed from solid-js@2.0.0-rc.3, which re-exports each of
// these names from @solidjs/signals@2.0.0-rc.3 (`types/index.d.ts:1`).
//
// Byte-faithful, because every claim in this fixture rests on them:
//
// - `flush`    `dist/types/core/scheduler.d.ts:176-177` (both overloads)
// - `action`   `dist/types/core/action.d.ts:64`
//
// ("Byte-faithful" up to the one token an ambient `declare module` block
// forbids: the published files spell `export declare function`.) Both are
// identical on rc.3, rc.8 and rc.9 apart from their line numbers.

declare module "solid-js" {
  export function flush(): void;
  export function flush<T>(fn: () => T): T;
  export function action<Args extends any[], Y, R>(genFn: (...args: Args) => Generator<Y, R, any> | AsyncGenerator<Y, R, any>): (...args: Args) => Promise<R>;
}
