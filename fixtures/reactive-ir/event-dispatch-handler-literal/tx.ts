// Exported from its own module of a closed program; calls its argument during
// the call.
export function withTx(run: () => void): void {
  run();
}
