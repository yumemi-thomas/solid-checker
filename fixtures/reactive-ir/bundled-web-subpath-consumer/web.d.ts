declare module "@solidjs/web/server-functions" {
  export function GET<T extends (...args: any[]) => any>(fn: T): T;
}
