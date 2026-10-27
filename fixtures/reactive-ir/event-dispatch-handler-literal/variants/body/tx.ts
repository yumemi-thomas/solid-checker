export function withTx(run: () => void): void {
  run();
}
