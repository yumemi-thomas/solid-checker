// The declarations a consumer compiles against. The census decides from
// `index.js`; the positive fact also requires every declared result here to be
// a primitive alone, which `widened` does not state.
export declare const trueFn: () => boolean;
export declare const voidFn: () => void;
export declare function clamp(value: number, min: number, max: number): number;
export declare function label(flag: boolean): "on" | "off";
export declare function isObject(value: unknown): value is object;
export declare function sign(value: number): 1 | -1 | undefined;
export declare function box<T>(value: T): { value: T };
export declare function passThrough<T>(value: T): T;
export declare function annotatedBox(): number;
export declare function add(a: number, b: number): number;
export declare function widened(): number | object;
export declare function limit(): number;
export declare function reassignedLet(key: string): number;
export declare const toNumber: (raw: string) => number;
export declare function toLabel(value: unknown): string;
export declare function shadowed(Number: (value: unknown) => number, value: unknown): number;
export declare function wrapped(value: unknown): number;
