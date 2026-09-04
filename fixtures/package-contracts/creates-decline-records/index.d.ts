// The declarations a consumer compiles against. Every export is a plain
// function and nothing here says anything about `creates`: the domain is
// decided — or, in this fixture, declined — by a walk of `index.js`.
export declare function dialectSilent(value: number): void;
export declare function viaSilentHelper(value: number): void;
export declare function unresolvedCallee(value: number): number;
