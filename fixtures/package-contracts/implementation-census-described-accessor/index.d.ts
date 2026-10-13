// `access` and `compare` exactly as `@solid-primitives/utils@7.0.0-next.4`
// declares them, with its `types.d.ts` helpers inlined.
type Accessor<T> = () => T;
type MaybeAccessor<T> = T | Accessor<T>;
type MaybeAccessorValue<T extends MaybeAccessor<any>> = T extends (() => any) ? ReturnType<T> : T;
export declare const access: <T extends MaybeAccessor<any>>(v: T) => MaybeAccessorValue<T>;
export declare const compare: (a: any, b: any) => number;
export declare const deferredRead: (v: any) => () => number;
export declare const nestedRead: (v: any) => number;
export declare const defaultRead: (v: any, w?: any) => number;
export declare const readAndCoerce: (v: any) => any;
type Point = [number, number];
type Polygon = Point[];
export declare function destructureAndMeasure(point: Point, polygon: Polygon): number;
export declare function isPointInPolygon(point: Point, polygon: Polygon): boolean;
export declare function plainArithmetic(a: number, b: number): number;
export declare const callAndAdd: (f: () => number, n: number) => number;
export declare const isNonNullable: <T>(i: T) => i is NonNullable<T>;
