// A local parameter's spelling supplies no primitive or derived-origin fact.
export function unrelated(createMemo: (callback: () => Promise<number>) => unknown) {
  return createMemo(async () => 1);
}

export function discarded(read: () => number) {
  void Promise.resolve().then(async () => read());
  return 9;
}

export function used(read: () => number) {
  return Promise.resolve().then(async () => read());
}
