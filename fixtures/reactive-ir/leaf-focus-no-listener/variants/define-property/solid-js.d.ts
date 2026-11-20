type Attributes<T> = {
  ref?: (node: T) => void;
  onClick?: (event: MouseEvent) => void;
  onFocus?: (event: FocusEvent) => void;
  onBlur?: (event: FocusEvent) => void;
  onFocusIn?: (event: FocusEvent) => void;
  onFocusOut?: (event: FocusEvent) => void;
  onChange?: (event: Event) => void;
  onInput?: (event: Event) => void;
  [attribute: string]: unknown;
};

declare namespace JSX {
  interface IntrinsicElements {
    div: Attributes<HTMLDivElement>;
    input: Attributes<HTMLInputElement>;
    dialog: Attributes<HTMLDialogElement>;
    "x-field": Attributes<HTMLElement>;
  }
}

declare module "solid-js" {
  export function createSignal<T>(value: T): [() => T, (value: T | ((prev: T) => T)) => T];
  // Byte-faithful to `@solidjs/signals@2.0.0-rc.21`'s
  // `createTrackedEffect(compute: () => void | (() => void), options?)` and `onSettled(callback: () => void | (() => void))`.
  // Do not loosen these: a stub that returns `unknown` manufactures cleanup-return defects no real project can produce
  // (fixtures/tsc-oracle/README.md).
  export function createTrackedEffect(callback: () => void | (() => void)): void;
  export function onSettled(callback: () => void | (() => void)): void;
  export function onCleanup(callback: () => void): void;
}
