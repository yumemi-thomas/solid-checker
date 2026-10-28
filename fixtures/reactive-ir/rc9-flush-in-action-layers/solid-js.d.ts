// Declarations transcribed from solid-js@2.0.0-rc.9, which re-exports each of
// these names from @solidjs/signals@2.0.0-rc.9 (`types/index.d.ts:1`).
//
// Byte-faithful, because every claim in this fixture rests on them:
//
// - `flush`    `dist/types/core/scheduler.d.ts:337-338` (both overloads)
// - `action`   `dist/types/core/action.d.ts:71`
// - `untrack`  `dist/types/core/core.d.ts:105`
//
// ("Byte-faithful" up to the one token an ambient `declare module` block
// forbids: the published files spell `export declare function`.) The three are
// identical on rc.3, rc.8 and rc.9 apart from their line numbers, which is
// why `tsc` cannot tell a release that throws from one that does not.
//
// `JSX` is this file's, reduced to the one element the fixture renders.

declare namespace JSX {
  interface IntrinsicElements {
    button: { onClick?: unknown; children?: any };
  }
  interface Element {}
}

declare module "solid-js" {
  export function flush(): void;
  export function flush<T>(fn: () => T): T;
  export function action<Args extends any[], Y, R>(genFn: (...args: Args) => Generator<Y, R, any> | AsyncGenerator<Y, R, any>): (...args: Args) => Promise<R>;
  export function untrack<T>(fn: () => T, strictReadLabel?: string | false): T;
}
