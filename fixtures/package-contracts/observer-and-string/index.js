export function observeIntersection(callback) {
  return new IntersectionObserver(callback);
}

export function observeResize(callback) {
  return new ResizeObserver(callback);
}

export function observeMutation(callback) {
  return new MutationObserver(callback);
}

export function observePerformance(callback) {
  return new PerformanceObserver(callback);
}
