// The JSX namespace for the stubs of the published 2.0.0-rc.21 signatures this fixture uses: `createContext`, `useContext`, `createTrackedEffect`, `flush`, `Errored`
// (`solid-js` and `@solidjs/signals` types), and `@solidjs/web`'s `dynamic` (`types/index.d.ts:157`: a `Component` whatever the source returns).
declare namespace JSX {
  interface IntrinsicElements {
    div: { children?: unknown };
    p: { children?: unknown };
  }
  type Element = object | string | number | boolean | null | undefined;
  interface ElementChildrenAttribute {
    children: {};
  }
}
