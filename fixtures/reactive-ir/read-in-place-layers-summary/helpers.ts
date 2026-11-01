// A module helper that calls its callback in its own body on every completion:
// the callback runs on the stack of the `h2` call (ADR 0469's layer).
export function h2(cb: () => unknown) {
  cb();
  return 1;
}
