declare namespace JSX {
  interface IntrinsicElements { p: { children?: unknown } }
  type Element = object | string | number | boolean | null | undefined;
  interface ElementChildrenAttribute { children: {} }
}

// The project declares the modules itself, ambiently, in one of its own files: the installed `solid-js` carries no declarations here (as with a workspace whose `paths` point at
// a local declaration file). The callee's declaration is therefore under no installed package directory, and the effect-phase walk, which asks the package the callee is
// declared in, has none to ask.
declare module "solid-js" {
  export function createEffect<T>(
    compute: () => T,
    apply: (value: T, previous?: T) => (() => void) | void,
  ): void;
  export function createMemo<T>(compute: (v: undefined | T) => T): () => T;
}

declare module "@solidjs/web" {
  export function render(code: () => JSX.Element, element: object): () => void;
}
