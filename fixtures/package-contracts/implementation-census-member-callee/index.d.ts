// `callHandler` and `composeEventHandlers` as `@kobalte/utils@2.0.0-alpha.0`
// declares them, with `@solidjs/web`'s `JSX.EventHandlerUnion` inlined and the
// DOM's `Event` and `Element` replaced by local stand-ins, so the declarations
// resolve without the DOM library. Every signature a rule's proof depends on
// is the published one: a handler is a function of the event or a
// `[function, data]` pair, or undefined.
interface BaseEvent {
  readonly defaultPrevented: boolean;
}
interface TargetElement {
  readonly tagName: string;
}
interface EventHandler<T, E extends BaseEvent> {
  (e: E & { currentTarget: T; target: TargetElement }): void;
}
interface BoundEventHandler<
  T,
  E extends BaseEvent,
  EHandler extends EventHandler<T, any> = EventHandler<T, E>
> {
  0: (data: any, ...e: Parameters<EHandler>) => void;
  1: any;
}
type EventHandlerUnion<
  T,
  E extends BaseEvent,
  EHandler extends EventHandler<T, any> = EventHandler<T, E>
> = EHandler | BoundEventHandler<T, E, EHandler>;
export declare function callHandler<T, E extends BaseEvent>(
  event: E & { currentTarget: T; target: TargetElement },
  handler: EventHandlerUnion<T, E> | undefined
): boolean;
export declare function callBound<T, E extends BaseEvent>(
  event: E & { currentTarget: T; target: TargetElement },
  handler: BoundEventHandler<T, E>
): void;
export declare function composeEventHandlers<T>(
  handlers: Array<EventHandlerUnion<T, any> | undefined>
): (event: any) => void;
export declare function computedKey(h: Record<string, () => void>, k: string): void;
export declare function deferredMember(h: [() => void]): () => void;
export declare function writtenBinding(h?: [() => void]): void;
export declare function stringKey(h: { run: () => void }): void;
