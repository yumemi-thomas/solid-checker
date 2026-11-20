import { CalledDirectly } from "./App";

// A test imports the product. Its own sites keep every writer (the runner's setup loads them); the product's are judged against what the page loads.
export function TestReversesAndCalls() {
  new Float32Array(1).reverse();
  CalledDirectly();
  return null;
}
