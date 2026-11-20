declare namespace JSX {
  interface IntrinsicElements {
    p: { children?: unknown };
    button: { onClick?: () => void; children?: unknown };
  }
  // Reduced from `solid-js@2.0.0-rc.21` `types/types.d.ts:5-14` (`RenderedElement`, `Element`, `ArrayElement`): an element is any object that is no function, so a Promise is one and
  // an `async` fallback type-checks against the real typings (Chrome: fixtures/runtime-oracle/errored-fallback-async, typingErrors 0).
  type RenderedElement = object & { readonly call?: never; readonly apply?: never; readonly bind?: never };
  interface ArrayElement extends Array<Element> {}
  type Element = RenderedElement | ArrayElement | (string & {}) | number | boolean | null | undefined;
  interface ElementChildrenAttribute { children: {} }
}
