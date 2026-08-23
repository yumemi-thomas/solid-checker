import { describedValue, undescribedValue } from "partial-contract-package";

export function forwardDescribed(callback) {
  return describedValue(callback);
}

export function forwardUndescribed(callback) {
  return undescribedValue(callback);
}
