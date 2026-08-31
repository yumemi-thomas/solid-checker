// A real module reached through the same wildcard key as the assets beside it.
// It has no declaration sibling, so the runtime file is its own declaration
// source; the case must certify exactly as an explicit entrypoint would.
export function render(callback) {
  callback();
}
