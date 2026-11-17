// A boxing helper in another module: the walk reads the helper in the caller's file only (open).
export function boxedElsewhere(cb: () => void) {
  return { cb };
}
