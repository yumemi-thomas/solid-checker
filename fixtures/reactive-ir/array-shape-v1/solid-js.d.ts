// The handler attribute types are byte-faithful to solid-js@1.9.14's
// `types/jsx.d.ts`, because `v1/no-array-handlers`' proof depends on them: the
// rule fires exactly where a value satisfies `BoundEventHandler`, which is an
// *interface with numbered members*, not a tuple type. A permissive
// `IntrinsicElements` index signature would erase the contextual typing that
// gives an inline `[handler, data]` literal its fixed slots, and the fixture
// would silently stop exercising the path it exists for.
//
// Everything else is reduced. Only these signatures are load-bearing.
declare namespace JSX {
  interface EventHandler<T, E extends Event> {
    (
      e: E & {
        currentTarget: T;
        target: Element;
      },
    ): void;
  }

  interface BoundEventHandler<
    T,
    E extends Event,
    EHandler extends EventHandler<T, any> = EventHandler<T, E>,
  > {
    0: (data: any, ...e: Parameters<EHandler>) => void;
    1: any;
  }

  type EventHandlerUnion<
    T,
    E extends Event,
    EHandler extends EventHandler<T, any> = EventHandler<T, E>,
  > = EHandler | BoundEventHandler<T, E, EHandler>;

  interface EventHandlerWithOptions<T, E extends Event, EHandler = EventHandler<T, E>>
    extends AddEventListenerOptions {
    handleEvent: EHandler;
  }

  // The `on:` namespace has no bound-handler arm at all. That is why every
  // array and tuple there is TS2322 and the rule's `on:` arm was removed.
  type EventHandlerWithOptionsUnion<
    T,
    E extends Event,
    EHandler extends EventHandler<T, any> = EventHandler<T, E>,
  > = EHandler | EventHandlerWithOptions<T, E, EHandler>;

  interface HandlerAttributes<T> {
    onClick?: EventHandlerUnion<T, MouseEvent>;
    onclick?: EventHandlerUnion<T, MouseEvent>;
    onMouseOver?: EventHandlerUnion<T, MouseEvent>;
    "on:click"?: EventHandlerWithOptionsUnion<T, MouseEvent>;
  }

  interface IntrinsicElements {
    button: HandlerAttributes<HTMLButtonElement>;
    div: HandlerAttributes<HTMLDivElement>;
    [name: string]: any;
  }
  interface Element {}
}

declare module "solid-js" {
  export function createSignal<T>(v: T): [() => T, (n: T) => void];
}
