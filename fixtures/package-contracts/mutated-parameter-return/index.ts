// Activate the existing structured-return pass, as the real bundle does.
const ancillary = { seed: 0 };
export function Direct<T>(value: T): T { return value; }
export function Mutated(value: number): number { value = 0; return value; }
export function Updated(value: number): number { value++; return value; }
export function Nested(value: number): number { function change() { value = 0; } change(); return value; }
export function Alias<T>(value: T): T { const alias = value; return alias; }
export function Defaulted(value = 0): number { return value; }
export function Destructured<T>({ value }: { value: T }): T { return value; }
export async function Async<T>(value: T): Promise<T> { return value; }
