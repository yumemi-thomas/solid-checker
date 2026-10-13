// Cross-file callees: one invokes its parameter during the call, one returns
// a closure that invokes it later.
export function runNow(callback: () => void): void {
  callback();
}

export function runLater(callback: () => void): () => void {
  return () => callback();
}
