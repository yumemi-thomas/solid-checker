import { choose, isEven, reset } from "./helpers.js";

export { choose, isEven, reset };
export { isOdd } from "./helpers.js";

// The control: the same shape as `isEven`, declared in the entry file itself.
export function isZero(value) {
  return value === 0;
}
