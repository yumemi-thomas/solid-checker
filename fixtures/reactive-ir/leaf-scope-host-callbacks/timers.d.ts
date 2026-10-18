// Project-owned ambient overloads: a selected signature outside the default
// library must retain the identity of the actual merged global binding.
// These are not a package stub or a claim about a package implementation.
declare function setInterval<TArgs extends any[]>(
  callback: (...args: TArgs) => void,
  ms?: number,
  ...args: TArgs
): number;
declare function setTimeout<TArgs extends any[]>(
  callback: (...args: TArgs) => void,
  ms?: number,
  ...args: TArgs
): number;
declare function queueMicrotask(callback: () => void): void;

// Byte-faithful to lib.dom.d.ts's fetch declaration; project-owned overload.
declare function fetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response>;
