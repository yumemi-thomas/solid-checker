import { observe } from "dep-ext";

// Another module's helper, reached through two hops.
function inner(source: () => number): void {
  observe(source);
}

export function relay(source: () => number): void {
  inner(source);
}
