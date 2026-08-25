function loadModule(url) {
  return import(url);
}

export function load(url) {
  return loadModule(url);
}

export const loadLater = url => load(url);

export function identity(value) {
  return value;
}

// Keep structured-return discovery active so the independently certifiable
// identity summary is part of this fixture's assertion.
function structuredSeed(value) {
  return [value];
}

void structuredSeed;
