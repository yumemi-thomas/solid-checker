declare module "@solidjs/web/server-functions" {
  export function GET<F extends (...args: any[]) => any>(fn: F): F;
  export function live<F extends (...args: any[]) => any>(fn: F): F;
  export function withMeta<F extends (...args: any[]) => any>(fn: F, meta: object): F;
}
declare module "@solidjs/router" {
  export function query<F extends (...args: any[]) => any>(fn: F, name: string): F;
  export function action<F extends (...args: any[]) => any>(fn: F): F;
  export function liveQuery<F extends (...args: any[]) => any>(fn: F, name: string): F;
}
declare module "some-other-package" {
  export function query<F extends (...args: any[]) => any>(fn: F, name: string): F;
}
