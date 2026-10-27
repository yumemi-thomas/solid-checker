// A helper that runs its callback during the call.
export function withTx(fn: () => void): void {
  fn();
}

// A helper that keeps its callback for later: nothing here says when, or from
// what stack, it runs.
const queue: Array<() => void> = [];
export function later(fn: () => void): void {
  queue.push(fn);
}

// A helper that hands its callback to a timer.
export function afterTick(fn: () => void): void {
  setTimeout(fn, 0);
}
