export function logged<F extends (...args: any[]) => any>(fn: F): F {
  return fn;
}
