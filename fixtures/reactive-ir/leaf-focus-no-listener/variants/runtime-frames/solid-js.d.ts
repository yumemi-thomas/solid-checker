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

// Byte-faithful to the declarations of `@solidjs/web@2.0.0-rc.21` (`types/client.d.ts`), the ones these programs call.
declare module "@solidjs/web" {
  export function addEvent(node: Element, name: string, handler: EventListener | EventListenerObject | (EventListenerObject & AddEventListenerOptions), delegate: boolean): EventListener | EventListenerObject | void;
  export function delegateEvents(eventNames: string[]): void;
  export function setAttribute(node: Element, name: string, value: string): void;
  export function setProperty(node: Element, name: string, value: any): void;
  export function template(html: string, flag?: 1 | 2): () => Element;
  export function spread<T>(node: Element, accessor: T, skipChildren?: Boolean, skip?: (key: string) => boolean, name?: string, loc?: string): void;
  // Narrower than the real `Dynamic<T extends ValidComponent>(props: DynamicProps<T>)`: this stub declares no `JSX.Element` or `ValidComponent`.
  export function Dynamic(props: { component: string | null | undefined | false; ref?: (node: HTMLInputElement) => void; [key: string]: unknown }): unknown;
}
declare module "@solidjs/web/frames/client" {
  export function createFrame(boundary: Element): void;
}
