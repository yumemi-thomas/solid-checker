// The declarations a consumer compiles against. Nothing here says anything
// about `creates`, which is why the domain is decided by a census of `index.js`.
export declare function plainConsumer(callback: (value: number) => void): void;
export declare function callsDependency(value: string): string;
