// The declarations a consumer compiles against. Every export is a plain
// function and nothing here says anything about `creates`: the domain is
// decided — or, in this fixture, declined — by a walk of `index.js`.
export declare function dialectSilent(value: number): void;
export declare function viaSilentHelper(value: number): void;
export declare function unresolvedCallee(value: number): number;
// One export per remaining `unresolved-callee` shape. Nothing here is looser
// than the runtime module it declares: every signature is exactly the shape
// `index.js` implements, so no shape depends on a stub's slack.
export declare function memberPropertyUnresolved(value: number): unknown;
export declare function memberReceiverUnresolved(value: number): unknown;
export declare function computedMember(handlers: Array<() => void>, key: number): void;
export declare function parameterRooted(source: { read(): unknown }): unknown;
export declare function parameterAliasRooted(source: { read(): unknown }): unknown;
export declare function expressionCallee(value: number): number;
export declare function otherSyntax(promised: Promise<() => unknown>): Promise<unknown>;
