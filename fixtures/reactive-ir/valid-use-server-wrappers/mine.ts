export const helper = 1;
export function query<T>(fn: T, _name: string): T {
  return fn;
}
export function logged<T>(fn: T): T {
  return fn;
}
