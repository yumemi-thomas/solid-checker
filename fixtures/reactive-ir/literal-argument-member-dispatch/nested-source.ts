export interface NestedReader {
  source: { read(): number };
}

export function readNested(input: NestedReader): number {
  return input.source.read();
}

declare function retain(value: unknown): void;

export function escapingNested(input: NestedReader): number {
  retain(input.source);
  return input.source.read();
}

export function replacingNested(input: NestedReader): number {
  input.source = { read: () => 0 };
  return input.source.read();
}

export function defaultNested(input: NestedReader = { source: { read: () => 0 } }): number {
  return input.source.read();
}

export function callbackNested(input: { source: { read(cb: () => number): number } }, cb: () => number) {
  return input.source.read(cb);
}

export function indexed() { return 0; }
export namespace indexed {
  export const branch = { read: () => 1 };
}
export function computedNested(input: { outer: { source: typeof indexed } }, key: "branch") {
  return input.outer.source[key].read();
}
