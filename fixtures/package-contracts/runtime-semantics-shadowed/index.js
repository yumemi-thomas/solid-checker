class ResizeObserver {
  constructor(callback) {}
}

export function shadowedResizeObserver(callback) {
  return new ResizeObserver(callback);
}
