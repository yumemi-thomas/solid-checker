// `callHandler` as `@kobalte/utils@2.0.0-alpha.0` declares it, with
// `@solidjs/web`'s `JSX.EventHandlerUnion` inlined and the DOM's `Event` and
// `Element` replaced by local stand-ins, so the declarations resolve without
// the DOM library. Every signature a rule's proof depends on is the published
// one: a handler is a function of the event or a `[function, data]` pair, or
// undefined, and the return is the event's `defaultPrevented`.
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
export declare function readKey<T>(options: { key: T }): T;
export declare function firstOrUndefined<T>(list: readonly T[] | undefined): T | undefined;
export declare function stringKey<T>(options: { run: T }): T;
export declare function keyOrSelf<T extends { key: unknown }>(
  value: T,
  useKey: boolean
): T | T["key"];
export declare function writtenMember(options: { key: unknown }): number;
export declare function computedKey<T>(options: Record<string, T>, key: string): T;
export declare function writtenBinding<T>(options?: { key?: T }): T | undefined;
export declare function longerPath<T>(options: { inner: { key: T } }): T;
export declare function memberCall<T>(options: { key: () => T }): T;
export declare function readBeforeWrite<T>(options: { key: T | number }): T | number;
export declare function overclaimedUndefined<T>(options: { key: T }): T;
