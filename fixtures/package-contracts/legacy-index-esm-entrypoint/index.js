export function scheduleLegacy(callback) {
  queueMicrotask(callback);
}
