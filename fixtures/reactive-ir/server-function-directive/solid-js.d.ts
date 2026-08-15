declare module "@solidjs/web/server-functions" {
  export function GET<F extends (...args: any[]) => any>(fn: F): F;
}
