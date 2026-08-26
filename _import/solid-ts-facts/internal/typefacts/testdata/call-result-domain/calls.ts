declare function makeCount(): number;
declare function makeThunk(): () => void;
declare function make(): (() => void) | undefined;

export const count = makeCount();
export const thunk = makeThunk();
export const optional = make();
