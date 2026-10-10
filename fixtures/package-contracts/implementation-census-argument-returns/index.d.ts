// The declarations a consumer compiles against: exactly the shapes `index.js`
// implements.
export declare const asArray: <T>(value: T) => (T extends any[] ? T[number] : NonNullable<T>)[];
export declare function pick<L, R>(flag: boolean, left: L, right: R): L | R;
export declare function pairOrValue<T, U>(value: T, other?: U): T | [T, U];
export declare function reassigned(value: unknown): unknown;
export declare function withLiteral<T>(value: T): T | [T, number];
export declare function overclaimed<T>(value: T): T | [T];
export declare function viaCall<T>(value: T): T | unknown[];
// `access` and `accessWith` exactly as `@solid-primitives/utils@7.0.0-next.4`
// declares them, with its `types.d.ts` helpers inlined.
type AnyFunction = (...args: any[]) => any;
type Accessor<T> = () => T;
type MaybeAccessor<T> = T | Accessor<T>;
type MaybeAccessorValue<T extends MaybeAccessor<any>> = T extends (() => any) ? ReturnType<T> : T;
export declare function accessWith<T>(valueOrFn: T, ...args: T extends AnyFunction ? Parameters<T> : never): T extends AnyFunction ? ReturnType<T> : T;
export declare const access: <T extends MaybeAccessor<any>>(v: T) => MaybeAccessorValue<T>;
export declare const run: <T, R>(fn: (value: T) => R, value: T) => R;
export declare const wrap: <T>(value: T) => [T];
export declare const parenthesized: <T>(value: T) => T | [T];
export declare function optionalCall<T, R>(fn: (() => R) | undefined, value: T): T | R | undefined;
