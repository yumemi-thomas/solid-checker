const queue: Array<() => void> = [];

// Keeps its argument: nothing says when, or on whose stack, it runs.
export function later(fire: () => void): void {
  queue.push(fire);
}
