export declare function createThing<T>(read: () => T): { read: () => T };
export declare function wrapThing<T>(read: () => T): { read: () => T };
export declare function plain<T>(value: T): T;
