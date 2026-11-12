// Declarations transcribed from @solidjs/signals@2.0.0-rc.21: `flush` `dist/types/core/scheduler.d.ts:448-449`.
declare module "solid-js" {
  export function flush(): void;
  export function flush<T>(fn: () => T): T;
}
