export function noted(_inner: (fn: () => void) => void) {
  console.log("made");
  return (fn: () => void) => {
    fn();
  };
}

export function either(inner: (fn: () => void) => void, flag: boolean) {
  if (flag) return inner;
  return (fn: () => void) => {
    fn();
  };
}
