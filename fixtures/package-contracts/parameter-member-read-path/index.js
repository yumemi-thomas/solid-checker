// One segment: the whole access is `options.slice`, so the path is one long.
// This is the shape `parameter-member-read` already pins; it is repeated here
// as the control that the longer paths below must not disturb.
export function oneSegment(options, count) {
  return options.slice(count);
}

// Two segments. The read walks `source` and then `slice`, so the path is
// ["source", "slice"]. Rooting it at the last segment alone would claim the
// parameter has a `slice` property, which it does not -- and a consumer
// matches this path as a prefix of the observed access, so that claim can
// never be witnessed by any runtime.
export function twoSegments(options, count) {
  return options.source.slice(count);
}

// Three segments, to pin that the walk is not special-cased to depth two.
export function threeSegments(options, count) {
  return options.input.source.slice(count);
}

// A computed segment names no property. The longest exact prefix from the
// parameter is empty, so the row claims only "read through this parameter" --
// the weakest claim the model has, and the only sound one here. Guessing
// `slice` (the last segment) would assert a property of a value this package
// cannot see.
export function computedRoot(options, key, count) {
  return options[key].slice(count);
}

// A computed segment *inside* the chain truncates to the exact prefix that
// precedes it: `source` is proven, everything past `[key]` is not.
export function computedInside(options, key, count) {
  return options.source[key].slice(count);
}

// Two accesses through the same parameter that walk different paths. The row
// keeps one entry per parameter and can only name a path every access agrees
// on, so this one stays unnamed. Comparing last segments alone would have
// called both of these "slice" and published a single agreed path that neither
// access performs.
export function disagreeingPaths(options, count) {
  return options.source.slice(count).concat(options.other.slice(count));
}

// The negative control: the identical member chain on a value this module
// created. Nothing the caller supplies is read, so no row is published at all.
const moduleLocal = { source: [1, 2, 3] };

export function readModuleLocal() {
  return moduleLocal.source.slice(1);
}
