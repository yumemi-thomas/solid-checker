import { BaseError, ChildError, Watcher } from "./errors.ts";

export class DirectError extends Error {}

const AliasedWatcher = Watcher;

export function plainFunction(value: number): number {
  return value;
}

export const settings = { retries: 2 };

export { AliasedWatcher, BaseError, ChildError, Watcher };
