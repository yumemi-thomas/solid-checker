// A boxing helper in another module: ADR 1058 reads the helper in the caller's file only (open).
export function boxedElsewhere(cb: () => void) {
  return { cb };
}
