// Returns a function that runs its argument bare: not the function it is handed.
export function ignore(_inner: (fn: () => void) => void) {
  return (fn: () => void) => {
    fn();
  };
}

// Returns the function it is handed: still not proven (a call's result).
export function pass<T>(inner: T): T {
  return inner;
}

// Returns a function that never runs its argument.
export function drop(_inner: (fn: () => void) => void) {
  return (fn: () => void) => {
    void fn;
  };
}
