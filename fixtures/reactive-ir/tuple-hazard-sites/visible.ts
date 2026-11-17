// A write of an iterator key on a target nothing proves separate from the array intrinsics.
export function install(target: any, next: () => unknown) {
  target[Symbol.iterator] = next;
}
