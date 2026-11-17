// An iterator key written on an object literal the file made: proven separate from the array intrinsics.
export function own() {
  const target: { [Symbol.iterator]?: () => Iterator<number> } = {};
  target[Symbol.iterator] = function* () { yield 1; };
  return target;
}
