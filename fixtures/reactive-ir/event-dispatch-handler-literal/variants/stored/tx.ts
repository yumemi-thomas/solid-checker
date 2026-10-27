const queue: Array<() => void> = [];

// Keeps its argument: nothing says when, or on whose stack, it runs.
export function withTx(run: () => void): void {
  queue.push(run);
}
