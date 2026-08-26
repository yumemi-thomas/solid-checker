export function sameName(value) {
  return value;
}

// Keep structured-return discovery active in this focused fixture. The
// production package contains many tuple/object return shapes; without one,
// the analyzer intentionally skips the fixed-point pass that also discovers
// relational returns.
function structuredSeed(value) {
  return [value];
}

void structuredSeed;
