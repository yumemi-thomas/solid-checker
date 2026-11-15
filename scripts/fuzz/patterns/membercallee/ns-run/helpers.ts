export function run(cb: () => unknown) {
  cb();
  return 1;
}
export const kept: Array<() => unknown> = [];
export function keep(cb: () => unknown) {
  kept.push(cb);
  return 1;
}
