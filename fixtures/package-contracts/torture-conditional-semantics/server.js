export function schedule(callback) {
  queueMicrotask(callback);
}

export const mode = "server";
