export type OnData = (value: number) => number;
export declare function Parameter(onData: OnData): number;
export declare function ObjectPattern(props: { onData: OnData }): number;
export declare function MemberAlias(props: { onData: OnData }): number;
export declare function ArrayPattern(handlers: readonly [OnData]): number;
