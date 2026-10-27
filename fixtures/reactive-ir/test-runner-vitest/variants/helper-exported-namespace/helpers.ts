// Exported from its own module of a closed program: the program's references
// to it are its callers, and every one is a call in a test callback.
export function choose(target: HTMLElement) {
  target.dispatchEvent(new Event("change"));
}
