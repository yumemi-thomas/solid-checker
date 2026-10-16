import { createMemo, untrack } from "solid-js";

// Invokes its parameter only as the tracked compute of a memo it creates.
export function tracked<T>(read: () => T) {
  return createMemo(() => read());
}

// Also invokes it in its own body, untracked.
export function trackedAndEager<T>(read: () => T) {
  read();
  return createMemo(() => read());
}

// Invokes it inside an untracked callback.
export function untracked<T>(read: () => T) {
  return untrack(() => read());
}

// An async wrapper: the memo is created after the call returns.
export async function trackedLater<T>(read: () => T) {
  await Promise.resolve();
  return createMemo(() => read());
}

// Hands the parameter itself on, uncalled, to the memo.
export function forwarded<T>(read: () => T) {
  return createMemo(read);
}

// A typed application adapter around a source-visible tracked helper.
export const wrapped: typeof tracked = ((read: () => unknown) =>
  tracked(() => {
    const options = read();
    return options;
  })) as typeof tracked;

export const wrappedAndEager: typeof tracked = ((read: () => unknown) => {
  read();
  return tracked(() => read());
}) as typeof tracked;

export const wrappedSnapshot: typeof tracked = ((read: () => unknown) => {
  const snapshot = untrack(read);
  return () => snapshot;
}) as typeof tracked;

export const wrappedLater: typeof tracked = ((read: () => unknown) => {
  const later = () => read();
  return later;
}) as typeof tracked;

declare function external(read: () => unknown): () => unknown;
export const wrappedUnknown: typeof tracked = ((read: () => unknown) =>
  external(read)) as typeof tracked;
