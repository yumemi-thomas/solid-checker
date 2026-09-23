// The declarations a consumer compiles against: exactly the shapes `index.js`
// implements.
export declare const asArray: <T>(value: T) => (T extends any[] ? T[number] : NonNullable<T>)[];
export declare function pick<L, R>(flag: boolean, left: L, right: R): L | R;
export declare function pairOrValue<T, U>(value: T, other?: U): T | [T, U];
export declare function reassigned(value: unknown): unknown;
export declare function withLiteral<T>(value: T): T | [T, number];
export declare function overclaimed<T>(value: T): T | [T];
export declare function viaCall<T>(value: T): T | unknown[];
