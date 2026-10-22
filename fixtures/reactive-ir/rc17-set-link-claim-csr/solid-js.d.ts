// Declarations transcribed from solid-js@2.0.0-rc.17 and @solidjs/web@2.0.0-rc.17:
// `LinkClaimHandler` @solidjs/web `types/server.d.ts:438` (re-exported by
// `types/client.d.ts:77`), `setLinkClaim` `types/client.d.ts:131`. Reduced:
// `createSignal` keeps only its value overload (rc.17 signals
// `dist/types/signals.d.ts:302`), and `JSX` itself.

declare namespace JSX {
  interface IntrinsicElements {}
  interface Element {}
}

declare module "solid-js" {
  export type Accessor<T> = () => T;
  export type Setter<T> = (value: T) => T;
  export function createSignal<T>(value: T): [get: Accessor<T>, set: Setter<T>];
}

declare module "@solidjs/web" {
  export type LinkClaimHandler = (attrs: Readonly<Record<string, unknown>>) => string;
  export function setLinkClaim(_handler: LinkClaimHandler | undefined): boolean;
}
