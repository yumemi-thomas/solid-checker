export function schedule(callback) {
  queueMicrotask(callback);
}
